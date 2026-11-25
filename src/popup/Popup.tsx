import React, { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { X } from "lucide-react";
import { commands, type LLMPrompt } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Textarea } from "@/components/ui/Textarea";

// Writing-tools actions live with the post-processing prompts; the transcript
// cleanup prompt is the dictation default, not a popup action.
const HIDDEN_PROMPTS = new Set(["default_improve_transcriptions"]);

// On macOS and Linux the popup is a floating card beside the caret (see
// popup.rs): a transparent borderless window with the card drawn here, closed
// by Esc, the X, or a click outside.
const IS_CARD = /Mac|Linux/.test(navigator.userAgent);
const RESULT_ID = "popup-result";

const Popup: React.FC = () => {
  const { t } = useTranslation();
  // Every action reads `original` and writes `final`, so actions don't stack.
  const [original, setOriginal] = useState("");
  const [final, setFinal] = useState("");
  const [prompts, setPrompts] = useState<LLMPrompt[]>([]);
  const [custom, setCustom] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPrompts = async () => {
    const res = await commands.getAppSettings();
    if (res.status === "ok") {
      setPrompts(
        (res.data.post_process_prompts ?? []).filter(
          (p) => !HIDDEN_PROMPTS.has(p.id),
        ),
      );
    }
  };

  const reset = (text: string) => {
    setOriginal(text);
    setFinal(text);
    setError(null);
    setCustom("");
    document.getElementById(RESULT_ID)?.focus();
  };

  useEffect(() => {
    invoke<string>("popup_get_text").then(reset);
    loadPrompts();
    const unlisten = listen<string>("popup-text", (e) => {
      reset(e.payload);
      loadPrompts();
    });
    // Clicking back into the app you were typing in dismisses the card.
    const unfocus = IS_CARD
      ? getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (!focused) getCurrentWindow().hide();
        })
      : null;
    return () => {
      unlisten.then((f) => f());
      unfocus?.then((f) => f());
    };
  }, []);

  const run = useCallback(
    async (
      key: string,
      promptId: string | null,
      instruction: string | null,
    ) => {
      setBusy(key);
      setError(null);
      try {
        setFinal(
          await invoke<string>("popup_run_prompt", {
            text: original,
            promptId,
            custom: instruction,
          }),
        );
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(null);
      }
    },
    [original],
  );

  const insert = useCallback(() => {
    if (final.trim()) invoke("popup_paste", { text: final });
  }, [final]);

  const disabled = busy !== null || !original.trim();

  // Esc closes, ⌘↵ inserts, ⌘1–9 run the actions in order.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (e.key === "Escape") {
        e.preventDefault();
        getCurrentWindow().hide();
      } else if (mod && e.key === "Enter") {
        e.preventDefault();
        insert();
      } else if (mod && /^[1-9]$/.test(e.key) && !disabled) {
        const p = prompts[Number(e.key) - 1];
        if (p) {
          e.preventDefault();
          run(p.id, p.id, null);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [insert, run, prompts, disabled]);

  return (
    <div className={IS_CARD ? "h-screen p-px" : "h-screen"}>
      <div
        className={`h-full flex flex-col gap-2 p-3 bg-background text-text ${
          IS_CARD ? "rounded-xl border border-line-strong" : ""
        }`}
      >
        {/* Header: a borderless card has no title bar to move or close it by. */}
        <div
          data-tauri-drag-region
          className="relative flex items-center justify-end h-5 -mt-1 shrink-0"
        >
          {IS_CARD && (
            <div
              data-tauri-drag-region
              className="absolute left-1/2 -translate-x-1/2 h-1.5 w-10 rounded-full bg-line-strong opacity-60 cursor-grab"
            />
          )}
          <button
            type="button"
            aria-label={t("common.close")}
            title={t("common.close")}
            className="p-0.5 rounded-md opacity-60 hover:opacity-100 hover:bg-logo-primary/10 focus:outline-none"
            onClick={() => getCurrentWindow().hide()}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="flex flex-wrap gap-1.5">
          {prompts.map((p, i) => (
            <Button
              key={p.id}
              variant="secondary"
              size="sm"
              disabled={disabled}
              title={i < 9 ? `⌘${i + 1}` : undefined}
              onClick={() => run(p.id, p.id, null)}
            >
              {busy === p.id ? "…" : p.name}
            </Button>
          ))}
        </div>

        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (custom.trim()) run("custom", null, custom);
          }}
        >
          <input
            className="flex-1 min-w-0 px-3 py-1 text-sm bg-wash border border-line-strong rounded-md focus:outline-none focus:border-logo-primary"
            placeholder={t("popup.customPlaceholder")}
            value={custom}
            onChange={(e) => setCustom(e.target.value)}
          />
          <Button
            type="submit"
            variant="secondary"
            size="sm"
            disabled={disabled || !custom.trim()}
          >
            {busy === "custom" ? "…" : t("popup.apply")}
          </Button>
        </form>

        {error && <p className="text-xs text-red-500">{error}</p>}

        <Textarea
          id={RESULT_ID}
          className="flex-1 min-h-0 w-full font-normal resize-none"
          aria-label={t("popup.final")}
          value={final}
          onChange={(e) => setFinal(e.target.value)}
        />

        <div className="flex gap-2 justify-end">
          <Button
            variant="ghost"
            size="sm"
            disabled={final === original}
            onClick={() => setFinal(original)}
          >
            {t("popup.original")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={!final.trim()}
            onClick={() => navigator.clipboard.writeText(final)}
          >
            {t("popup.copy")}
          </Button>
          <Button
            variant="primary"
            size="sm"
            disabled={!final.trim()}
            title="⌘↵"
            onClick={insert}
          >
            {t("popup.paste")}
          </Button>
        </div>
      </div>
    </div>
  );
};

export default Popup;
