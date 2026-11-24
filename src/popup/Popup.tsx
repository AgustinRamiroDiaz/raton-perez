import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { commands, type LLMPrompt } from "@/bindings";
import { Button } from "@/components/ui/Button";
import { Textarea } from "@/components/ui/Textarea";

// Writing-tools actions live with the post-processing prompts; the transcript
// cleanup prompt is the dictation default, not a popup action.
const HIDDEN_PROMPTS = new Set(["default_improve_transcriptions"]);

const Popup: React.FC = () => {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [prompts, setPrompts] = useState<LLMPrompt[]>([]);
  const [custom, setCustom] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Results of "open in window" prompts; replacing prompts edit `text` instead.
  const [result, setResult] = useState<string | null>(null);

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

  useEffect(() => {
    invoke<string>("popup_get_text").then(setText);
    loadPrompts();
    const unlisten = listen<string>("popup-text", (e) => {
      setText(e.payload);
      setResult(null);
      setError(null);
      setCustom("");
      loadPrompts();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const run = async (
    key: string,
    promptId: string | null,
    instruction: string | null,
    openInWindow: boolean,
  ) => {
    setBusy(key);
    setError(null);
    try {
      const out = await invoke<string>("popup_run_prompt", {
        text,
        promptId,
        custom: instruction,
      });
      if (openInWindow) setResult(out);
      else setText(out);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const paste = (value: string) => invoke("popup_paste", { text: value });

  return (
    <div className="h-screen flex flex-col gap-3 p-4 bg-background text-text">
      <Textarea
        className="flex-1 w-full font-normal"
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label={t("popup.transcript")}
      />

      <div className="grid grid-cols-4 gap-2">
        {prompts.map((p) => (
          <Button
            key={p.id}
            variant="secondary"
            size="sm"
            disabled={busy !== null || !text.trim()}
            onClick={() => run(p.id, p.id, null, p.open_in_window ?? false)}
          >
            {busy === p.id ? "…" : p.name}
          </Button>
        ))}
      </div>

      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (custom.trim()) run("custom", null, custom, false);
        }}
      >
        <input
          className="flex-1 px-3 py-1 text-sm bg-wash border border-line-strong rounded-md focus:outline-none focus:border-logo-primary"
          placeholder={t("popup.customPlaceholder")}
          value={custom}
          onChange={(e) => setCustom(e.target.value)}
        />
        <Button
          type="submit"
          variant="secondary"
          size="sm"
          disabled={busy !== null || !custom.trim() || !text.trim()}
        >
          {busy === "custom" ? "…" : t("popup.apply")}
        </Button>
      </form>

      {error && <p className="text-sm text-red-500">{error}</p>}

      {result !== null && (
        <div className="flex flex-col gap-2 max-h-48">
          <pre className="overflow-auto whitespace-pre-wrap text-sm p-2 bg-wash rounded-md border border-line">
            {result}
          </pre>
          <div className="flex gap-2 justify-end">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => navigator.clipboard.writeText(result)}
            >
              {t("popup.copy")}
            </Button>
            <Button variant="secondary" size="sm" onClick={() => paste(result)}>
              {t("popup.pasteResult")}
            </Button>
          </div>
        </div>
      )}

      <div className="flex gap-2 justify-end">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => getCurrentWindow().hide()}
        >
          {t("popup.close")}
        </Button>
        <Button
          variant="primary"
          size="sm"
          disabled={!text.trim()}
          onClick={() => paste(text)}
        >
          {t("popup.paste")}
        </Button>
      </div>
    </div>
  );
};

export default Popup;
