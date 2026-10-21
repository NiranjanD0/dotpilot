import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Camera, FileDiff, History, RefreshCw, RotateCcw } from "lucide-react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { DiffDialog } from "@/components/diff-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { api } from "@/lib/api";
import type { SnapshotView } from "@/lib/types";

export function SnapshotsPage() {
  const [snapshots, setSnapshots] = useState<SnapshotView[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [restoreTarget, setRestoreTarget] = useState<SnapshotView | null>(null);
  const [diff, setDiff] = useState<{
    title: string;
    description: string;
    text: string;
  } | null>(null);

  const load = useCallback(async () => {
    try {
      setSnapshots(await api.listSnapshots());
    } catch (error) {
      toast.error("Could not load the snapshot history", {
        description: (error as Error).message,
      });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const create = async () => {
    setBusy(true);
    try {
      const hash = await api.createSnapshot(message.trim() || "Snapshot");
      toast.success("Snapshot created", { description: hash });
      setMessage("");
      await load();
    } catch (error) {
      toast.error("Could not create the snapshot", {
        description: (error as Error).message,
      });
    } finally {
      setBusy(false);
    }
  };

  const compare = async (snapshot: SnapshotView, index: number) => {
    // The newest snapshot is compared with the working tree, so pending
    // local edits show up. Older ones are compared with the snapshot before.
    const older = index + 1 < snapshots.length ? snapshots[index + 1] : undefined;
    try {
      const text = await api.diffSnapshot(snapshot.hash, older?.hash);
      setDiff({
        title: snapshot.subject,
        description: older
          ? `Changes introduced by this snapshot, compared with ${older.hash.slice(0, 7)}.`
          : "Changes in the working tree compared with this snapshot.",
        text,
      });
    } catch (error) {
      toast.error("Could not compare the snapshots", {
        description: (error as Error).message,
      });
    }
  };

  const restore = async () => {
    if (!restoreTarget) return;
    const target = restoreTarget;
    setRestoreTarget(null);
    try {
      await api.restoreSnapshot(target.hash);
      toast.success("Restored the store", {
        description: `The managed files now match ${target.hash.slice(0, 7)}.`,
      });
      await load();
    } catch (error) {
      toast.error("Could not restore the snapshot", {
        description: (error as Error).message,
      });
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-start justify-between gap-4 border-b px-8 py-6">
        <div>
          <h1 className="text-xl font-semibold">Snapshots</h1>
          <p className="text-sm text-muted-foreground">
            {snapshots.length} {snapshots.length === 1 ? "snapshot" : "snapshots"} in
            the store
          </p>
        </div>
        <Button
          variant="outline"
          size="icon"
          onClick={() => void load()}
          aria-label="Refresh"
        >
          <RefreshCw className="size-4" />
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-auto px-8 py-6">
        <div className="mb-6 flex gap-2">
          <Input
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void create();
            }}
            placeholder="Describe this snapshot…"
          />
          <Button onClick={() => void create()} disabled={busy}>
            <Camera className="size-4" />
            {busy ? "Creating…" : "Create snapshot"}
          </Button>
        </div>

        {snapshots.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
            <History className="size-8 text-muted-foreground" />
            <div>
              <p className="font-medium">No snapshots yet</p>
              <p className="text-sm text-muted-foreground">
                Create a snapshot to record the current state of the store.
              </p>
            </div>
          </div>
        ) : (
          <ul className="divide-y rounded-lg border">
            {snapshots.map((snapshot, index) => (
              <li key={snapshot.hash} className="flex items-center gap-4 p-4">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate font-medium">{snapshot.subject}</span>
                    {index === 0 && <Badge variant="secondary">latest</Badge>}
                  </div>
                  <div className="font-mono text-xs text-muted-foreground">
                    {snapshot.hash.slice(0, 7)}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => void compare(snapshot, index)}
                        aria-label="Show changes"
                      >
                        <FileDiff className="size-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {index + 1 < snapshots.length
                        ? "Show what this snapshot changed"
                        : "Show the changes not captured yet"}
                    </TooltipContent>
                  </Tooltip>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setRestoreTarget(snapshot)}
                        aria-label="Restore this snapshot"
                      >
                        <RotateCcw className="size-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Restore the store to this snapshot</TooltipContent>
                  </Tooltip>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      <ConfirmDialog
        open={restoreTarget !== null}
        onOpenChange={(open) => {
          if (!open) setRestoreTarget(null);
        }}
        title="Restore this snapshot?"
        description={
          restoreTarget
            ? `Every managed file will be replaced with the content of ${restoreTarget.hash.slice(0, 7)}. The current store contents stay in the Git history.`
            : ""
        }
        confirmLabel="Restore"
        destructive
        onConfirm={() => void restore()}
      />

      <DiffDialog
        open={diff !== null}
        onOpenChange={(open) => {
          if (!open) setDiff(null);
        }}
        title={diff?.title ?? ""}
        description={diff?.description}
        text={diff?.text ?? ""}
      />
    </div>
  );
}
