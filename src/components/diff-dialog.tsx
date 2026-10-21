import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import type { DiffLine } from "@/lib/types";

interface DiffDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  /** Line-level diff to render. Ignored when `text` is given. */
  lines?: DiffLine[];
  /** Raw unified diff to render, e.g. the output of `git diff`. */
  text?: string;
  added?: number;
  removed?: number;
}

function LineNumber({ value }: { value: number | null }) {
  return (
    <span className="w-12 shrink-0 pr-3 text-right tabular-nums text-muted-foreground/70 select-none">
      {value ?? ""}
    </span>
  );
}

function RawDiff({ text }: { text: string }) {
  const lines = text.split("\n");
  if (text.trim() === "") {
    return (
      <p className="px-4 py-8 text-center text-sm text-muted-foreground">
        No differences.
      </p>
    );
  }
  return (
    <pre className="overflow-auto p-3 font-mono text-xs leading-relaxed">
      {lines.map((line, index) => (
        <div
          key={index}
          className={cn(
            "px-1 whitespace-pre-wrap",
            line.startsWith("+") && "bg-added/15 text-added",
            line.startsWith("-") && "bg-removed/15 text-removed",
            line.startsWith("@@") && "text-primary",
          )}
        >
          {line || " "}
        </div>
      ))}
    </pre>
  );
}

export function DiffDialog({
  open,
  onOpenChange,
  title,
  description,
  lines,
  text,
  added = 0,
  removed = 0,
}: DiffDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl gap-3">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description && (
            <DialogDescription>{description}</DialogDescription>
          )}
        </DialogHeader>

        {(added > 0 || removed > 0) && (
          <div className="flex gap-2 text-xs">
            <span className="rounded-md bg-added/15 px-2 py-1 text-added">
              +{added} added
            </span>
            <span className="rounded-md bg-removed/15 px-2 py-1 text-removed">
              −{removed} removed
            </span>
          </div>
        )}

        <div className="max-h-[55vh] overflow-auto rounded-lg border bg-muted/30">
          {text !== undefined ? (
            <RawDiff text={text} />
          ) : (
            <pre className="p-3 font-mono text-xs leading-relaxed">
              {(lines ?? []).map((line, index) => (
                <div
                  key={index}
                  className={cn(
                    "flex px-1",
                    line.kind === "added" && "bg-added/15 text-added",
                    line.kind === "removed" && "bg-removed/15 text-removed",
                  )}
                >
                  <LineNumber value={line.originalLine} />
                  <LineNumber value={line.modifiedLine} />
                  <span className="w-4 shrink-0 select-none">
                    {line.kind === "added"
                      ? "+"
                      : line.kind === "removed"
                        ? "−"
                        : " "}
                  </span>
                  <span className="whitespace-pre-wrap">{line.text || " "}</span>
                </div>
              ))}
              {lines && lines.length === 0 && (
                <p className="px-3 py-6 text-center text-muted-foreground">
                  No differences.
                </p>
              )}
            </pre>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
