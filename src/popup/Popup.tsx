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
  };

  useEffect(() => {
    invoke<string>("popup_get_text").then(reset);
    loadPrompts();
    const unlisten = listen<string>("popup-text", (e) => {
      reset(e.payload);
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
  };

  const disabled = busy !== null || !original.trim();

  return (
    <div className="h-screen flex flex-col gap-3 p-4 bg-background text-text">
      <label className="flex-1 flex flex-col gap-1 min-h-0">
        <span className="text-xs font-semibold opacity-70">
          {t("popup.original")}
        </span>
        <Textarea
          className="flex-1 w-full font-normal resize-none"
          value={original}
          onChange={(e) => setOriginal(e.target.value)}
        />
      </label>

      <div className="grid grid-cols-4 gap-2">
        {prompts.map((p) => (
          <Button
            key={p.id}
            variant="secondary"
            size="sm"
            disabled={disabled}
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
          className="flex-1 px-3 py-1 text-sm bg-wash border border-line-strong rounded-md focus:outline-none focus:border-logo-primary"
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

      {error && <p className="text-sm text-red-500">{error}</p>}

      <label className="flex-1 flex flex-col gap-1 min-h-0">
        <span className="text-xs font-semibold opacity-70">
          {t("popup.final")}
        </span>
        <Textarea
          className="flex-1 w-full font-normal resize-none"
          value={final}
          onChange={(e) => setFinal(e.target.value)}
        />
      </label>

      <div className="flex gap-2 justify-end">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => getCurrentWindow().hide()}
        >
          {t("popup.close")}
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
          onClick={() => invoke("popup_paste", { text: final })}
        >
          {t("popup.paste")}
        </Button>
      </div>
    </div>
  );
};

export default Popup;
