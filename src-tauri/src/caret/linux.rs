//! Linux: asks AT-SPI, the desktop accessibility service, over D-Bus where the
//! caret is. Only X11 reports screen coordinates; Wayland apps report them
//! relative to their own window, so there this returns nothing.

use std::time::{Duration, Instant};
use zbus::blocking::{connection, Connection};
use zbus::zvariant::{DynamicDeserialize, DynamicType, OwnedObjectPath, OwnedValue, Value};

use super::{plausible, Anchor};

/// Per call, so an app that doesn't answer can't stall the lookup.
const CALL_TIMEOUT: Duration = Duration::from_millis(250);
/// For the whole search; past it the popup opens by the mouse pointer.
const SEARCH_BUDGET: Duration = Duration::from_millis(700);
const MAX_NODES: usize = 3000;

const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const TEXT: &str = "org.a11y.atspi.Text";
const COMPONENT: &str = "org.a11y.atspi.Component";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

// AtspiStateType bits.
const STATE_ACTIVE: u32 = 1;
const STATE_FOCUSED: u32 = 12;
const STATE_SHOWING: u32 = 25;
const COORD_SCREEN: u32 = 0;

/// An accessible object: its bus name and object path.
type Node = (String, OwnedObjectPath);

fn call<B, R>(conn: &Connection, node: &Node, iface: &str, method: &str, body: &B) -> Option<R>
where
    B: serde::Serialize + DynamicType,
    R: for<'d> DynamicDeserialize<'d>,
{
    let reply = conn
        .call_method(
            Some(node.0.as_str()),
            node.1.as_str(),
            Some(iface),
            method,
            body,
        )
        .ok()?;
    reply.body().deserialize::<R>().ok()
}

fn property<T: TryFrom<OwnedValue>>(
    conn: &Connection,
    node: &Node,
    iface: &str,
    name: &str,
) -> Option<T> {
    let value: OwnedValue = call(conn, node, PROPERTIES, "Get", &(iface, name))?;
    T::try_from(value).ok()
}

fn children(conn: &Connection, node: &Node) -> Vec<Node> {
    call(conn, node, ACCESSIBLE, "GetChildren", &()).unwrap_or_default()
}

fn has_state(conn: &Connection, node: &Node, state: u32) -> Option<bool> {
    let words: Vec<u32> = call(conn, node, ACCESSIBLE, "GetState", &())?;
    Some(
        words
            .get((state / 32) as usize)
            .is_some_and(|w| w & (1 << (state % 32)) != 0),
    )
}

/// Asks apps to expose their accessibility tree. Chromium, Electron (Slack,
/// VS Code) and Qt only do so while this is on.
pub fn enable_accessibility() {
    let Ok(session) = Connection::session() else {
        return;
    };
    let Ok(path) = OwnedObjectPath::try_from("/org/a11y/bus") else {
        return;
    };
    let bus: Node = ("org.a11y.Bus".into(), path);
    let _: Option<()> = call(
        &session,
        &bus,
        PROPERTIES,
        "Set",
        &("org.a11y.Status", "IsEnabled", Value::from(true)),
    );
}

fn connect() -> Option<Connection> {
    let session = Connection::session().ok()?;
    let bus: Node = (
        "org.a11y.Bus".into(),
        OwnedObjectPath::try_from("/org/a11y/bus").ok()?,
    );
    let address: String = call(&session, &bus, "org.a11y.Bus", "GetAddress", &())?;
    connection::Builder::address(address.as_str())
        .ok()?
        .method_timeout(CALL_TIMEOUT)
        .build()
        .ok()
}

/// Depth-first search of a window for its focused element, skipping hidden
/// branches.
fn find_focused(conn: &Connection, window: Node, deadline: Instant) -> Option<Node> {
    let mut stack = vec![window];
    let mut visited = 0;
    while let Some(node) = stack.pop() {
        visited += 1;
        if visited > MAX_NODES || Instant::now() > deadline {
            return None;
        }
        if has_state(conn, &node, STATE_FOCUSED) == Some(true) {
            return Some(node);
        }
        if visited > 1 && has_state(conn, &node, STATE_SHOWING) == Some(false) {
            continue;
        }
        let mut kids = children(conn, &node);
        kids.reverse();
        stack.extend(kids);
    }
    None
}

fn caret_rect(conn: &Connection, node: &Node) -> Option<Anchor> {
    let offset: i32 = property(conn, node, TEXT, "CaretOffset")?;
    // At the end of the text there is no character under the caret, so fall
    // back to the right edge of the one before it.
    for (at, after) in [(offset, false), (offset - 1, true)] {
        if at < 0 {
            continue;
        }
        let Some((x, y, w, h)) = call::<_, (i32, i32, i32, i32)>(
            conn,
            node,
            TEXT,
            "GetCharacterExtents",
            &(at, COORD_SCREEN),
        ) else {
            continue;
        };
        let anchor = Anchor {
            x: f64::from(if after { x + w } else { x }),
            y: f64::from(y),
            width: 0.0,
            height: f64::from(h),
        };
        if plausible(&anchor) {
            return Some(anchor);
        }
    }
    None
}

fn element_rect(conn: &Connection, node: &Node) -> Option<Anchor> {
    let (x, y, w, h): (i32, i32, i32, i32) =
        call(conn, node, COMPONENT, "GetExtents", &(COORD_SCREEN,))?;
    Some(Anchor {
        x: f64::from(x),
        y: f64::from(y),
        width: f64::from(w),
        height: f64::from(h),
    })
    .filter(plausible)
}

/// Returns the caret's rectangle in the focused element of the active window,
/// or the element's own frame when it has no caret to report.
pub fn locate() -> Option<Anchor> {
    if crate::utils::is_wayland() {
        return None;
    }
    let conn = connect()?;
    let deadline = Instant::now() + SEARCH_BUDGET;
    let root: Node = (
        "org.a11y.atspi.Registry".into(),
        OwnedObjectPath::try_from("/org/a11y/atspi/accessible/root").ok()?,
    );
    for app in children(&conn, &root) {
        for window in children(&conn, &app) {
            if Instant::now() > deadline {
                return None;
            }
            if has_state(&conn, &window, STATE_ACTIVE) != Some(true) {
                continue;
            }
            let focused = find_focused(&conn, window, deadline)?;
            return caret_rect(&conn, &focused).or_else(|| element_rect(&conn, &focused));
        }
    }
    None
}
