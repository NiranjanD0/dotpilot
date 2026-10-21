import { useState } from "react";
import { FolderOpen, FileUp } from "lucide-react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

interface TrackDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onTrack: (path: string) => Promise<void>;
}

export function TrackDialog({
  open,
  onOpenChange,
  onTrack,
}: TrackDialogProps) {
  const [path, setPath] = useState("");
  const [directory, setDirectory] = useState(false);
  const [busy, setBusy] = useState(false);

  const reset = () => {
    setPath("");
    setDirectory(false);
    setBusy(false);
  };

  const browse = async () => {
    const selected = await openFileDialog({
      directory,
      multiple: false,
      title: directory ? "Choose a directory" : "Choose a file",
    });
    if (typeof selected === "string") {
      setPath(selected);
    }
  };

  const submit = async () => {
    if (!path.trim()) return;
    setBusy(true);
    try {
      await onTrack(path.trim());
      reset();
      onOpenChange(false);
    } catch {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Track a path</DialogTitle>
          <DialogDescription>
            The file is moved into the store and replaced with a symlink. The
            original is kept as a backup.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="flex gap-2">
            {(
              [
                { label: "File", value: false, icon: FileUp },
                { label: "Folder", value: true, icon: FolderOpen },
              ] as const
            ).map((option) => (
              <button
                key={option.label}
                type="button"
                onClick={() => {
                  setDirectory(option.value);
                  setPath("");
                }}
                className={cn(
                  "flex flex-1 items-center justify-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors",
                  directory === option.value
                    ? "border-primary bg-primary/10 font-medium text-primary"
                    : "text-muted-foreground hover:bg-accent",
                )}
              >
                <option.icon className="size-4" />
                {option.label}
              </button>
            ))}
          </div>

          <div className="space-y-2">
            <Label htmlFor="track-path">Path</Label>
            <div className="flex gap-2">
              <Input
                id="track-path"
                value={path}
                onChange={(event) => setPath(event.target.value)}
                placeholder="~/.config/nvim"
                spellCheck={false}
                autoFocus
              />
              <Button type="button" variant="outline" onClick={browse}>
                Browse…
              </Button>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={submit} disabled={!path.trim() || busy}>
            {busy ? "Tracking…" : "Track path"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
