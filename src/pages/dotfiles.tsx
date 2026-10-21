import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  FileDiff,
  FileText,
  Folder,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { DiffDialog } from "@/components/diff-dialog";
import { TrackDialog } from "@/components/track-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { api } from "@/lib/api";
import type { EntryView, TextDiffResult } from "@/lib/types";

function basename(path: string) {
  const parts = path.split("/").filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}

export function DotfilesPage() {
  const [entries, setEntries] = useState<EntryView[]>([]);
  const [loading, setLoading] = useState(true);
  const [trackOpen, setTrackOpen] = useState(false);
  const [untrackTarget, setUntrackTarget] = useState<EntryView | null>(null);
  const [purge, setPurge] = useState(false);
  const [diff, setDiff] = useState<{
    entry: EntryView;
    result: TextDiffResult;
  } | null>(null);

  const load = useCallback(async () => {
    try {
      setEntries(await api.listEntries());
    } catch (error) {
      toast.error("Could not list managed paths", {
        description: (error as Error).message,
      });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const track = async (path: string) => {
    try {
      const result = await api.trackPath(path);
      toast.success(`Tracking ${basename(result.entry.target)}`, {
        description: result.backup
          ? `The original was kept at ${result.backup}`
          : undefined,
      });
      await load();
    } catch (error) {
      toast.error("Could not track the path", {
        description: (error as Error).message,
      });
      throw error;
    }
  };

  const syncToStore = async (entry: EntryView) => {
    try {
      const changed = await api.syncToStore(entry.id);
      toast.success(
        changed
          ? `Captured local changes to ${basename(entry.target)}`
          : `${basename(entry.target)} is already in sync`,
      );
      await load();
    } catch (error) {
      toast.error("Could not capture the changes", {
        description: (error as Error).message,
      });
    }
  };

  const syncFromStore = async (entry: EntryView) => {
    try {
      const backup = await api.syncFromStore(entry.id);
      toast.success(`Restored ${basename(entry.target)} from the store`, {
        description: backup ? `The previous version was kept at ${backup}` : undefined,
      });
      await load();
    } catch (error) {
      toast.error("Could not restore the file", {
        description: (error as Error).message,
      });
    }
  };

  const previewDiff = async (entry: EntryView) => {
    try {
      const result = await api.diffEntry(entry.id);
      setDiff({ entry, result });
    } catch (error) {
      toast.error("Could not compare the file", {
        description: (error as Error).message,
      });
    }
  };

  const untrack = async () => {
    if (!untrackTarget) return;
    const target = untrackTarget;
    setUntrackTarget(null);
    try {
      const restored = await api.untrackPath(target.id, purge);
      toast.success(`Stopped tracking ${basename(target.target)}`, {
        description: restored ? `A real file was left at ${restored}` : undefined,
      });
      await load();
    } catch (error) {
      toast.error("Could not stop tracking", {
        description: (error as Error).message,
      });
    } finally {
      setPurge(false);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-start justify-between gap-4 border-b px-8 py-6">
        <div>
          <h1 className="text-xl font-semibold">Dotfiles</h1>
          <p className="text-sm text-muted-foreground">
            {loading
              ? "Loading…"
              : `${entries.length} managed ${entries.length === 1 ? "path" : "paths"}`}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="icon"
            onClick={() => void load()}
            aria-label="Refresh"
          >
            <RefreshCw className="size-4" />
          </Button>
          <Button onClick={() => setTrackOpen(true)}>
            <Plus className="size-4" />
            Track path
          </Button>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-auto px-8 py-6">
        {entries.length === 0 && !loading ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <FileText className="size-8 text-muted-foreground" />
            <div>
              <p className="font-medium">No paths are tracked yet</p>
              <p className="text-sm text-muted-foreground">
                Track a file or directory to start managing it.
              </p>
            </div>
            <Button onClick={() => setTrackOpen(true)}>
              <Plus className="size-4" />
              Track path
            </Button>
          </div>
        ) : (
          <div className="rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Path</TableHead>
                  <TableHead>Kind</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Added</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {entries.map((entry) => (
                  <TableRow key={entry.id}>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        {entry.kind === "directory" ? (
                          <Folder className="size-4 text-muted-foreground" />
                        ) : (
                          <FileText className="size-4 text-muted-foreground" />
                        )}
                        <div className="min-w-0">
                          <div className="font-medium">{basename(entry.target)}</div>
                          <div className="max-w-[28rem] truncate font-mono text-xs text-muted-foreground">
                            {entry.target}
                          </div>
                        </div>
                      </div>
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {entry.kind}
                    </TableCell>
                    <TableCell>
                      {!entry.present ? (
                        <Badge variant="destructive">missing</Badge>
                      ) : entry.linked ? (
                        <Badge variant="success">linked</Badge>
                      ) : (
                        <Badge variant="outline">detached</Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {formatDate(entry.addedAt)}
                    </TableCell>
                    <TableCell>
                      <div className="flex justify-end gap-1">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => void syncToStore(entry)}
                              aria-label="Capture local changes"
                            >
                              <ArrowUpFromLine className="size-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            Capture local changes into the store
                          </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => void syncFromStore(entry)}
                              aria-label="Restore from the store"
                            >
                              <ArrowDownToLine className="size-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            Restore the file from the store
                          </TooltipContent>
                        </Tooltip>
                        {entry.kind === "file" && (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                variant="ghost"
                                size="icon"
                                onClick={() => void previewDiff(entry)}
                                aria-label="Show changes"
                              >
                                <FileDiff className="size-4" />
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent>
                              Compare with the copy in the store
                            </TooltipContent>
                          </Tooltip>
                        )}
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => setUntrackTarget(entry)}
                              aria-label="Stop tracking"
                            >
                              <Trash2 className="size-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>Stop tracking this path</TooltipContent>
                        </Tooltip>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      <TrackDialog
        open={trackOpen}
        onOpenChange={setTrackOpen}
        onTrack={track}
      />

      <ConfirmDialog
        open={untrackTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setUntrackTarget(null);
            setPurge(false);
          }
        }}
        title="Stop tracking this path?"
        description={
          untrackTarget
            ? `${untrackTarget.target} will be replaced by a real copy of the file held in the store.`
            : ""
        }
        confirmLabel="Stop tracking"
        destructive
        onConfirm={() => void untrack()}
      >
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          <input
            type="checkbox"
            checked={purge}
            onChange={(event) => setPurge(event.target.checked)}
            className="size-4 accent-primary"
          />
          Also delete the copy kept in the store
        </label>
      </ConfirmDialog>

      <DiffDialog
        open={diff !== null}
        onOpenChange={(open) => {
          if (!open) setDiff(null);
        }}
        title={diff ? basename(diff.entry.target) : ""}
        description={
          diff
            ? "Local changes compared with the copy held in the store."
            : undefined
        }
        lines={diff?.result.lines}
        added={diff?.result.added}
        removed={diff?.result.removed}
      />
    </div>
  );
}
