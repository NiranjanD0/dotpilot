import type { ReactNode } from "react";
import { FileCog, History, ShieldAlert, Terminal } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type { EnvironmentSummary } from "@/lib/types";

export type View = "dotfiles" | "snapshots";

interface AppShellProps {
  view: View;
  onViewChange: (view: View) => void;
  environment: EnvironmentSummary | null;
  gitAvailable: boolean | null;
  children: ReactNode;
}

const NAV_ITEMS: { id: View; label: string; icon: typeof FileCog }[] = [
  { id: "dotfiles", label: "Dotfiles", icon: FileCog },
  { id: "snapshots", label: "Snapshots", icon: History },
];

export function AppShell({
  view,
  onViewChange,
  environment,
  gitAvailable,
  children,
}: AppShellProps) {
  return (
    <div className="flex h-full">
      <aside className="flex w-64 shrink-0 flex-col border-r bg-card">
        <div className="flex items-center gap-3 px-5 py-5">
          <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Terminal className="size-5" />
          </div>
          <div>
            <div className="text-sm font-semibold">DotPilot</div>
            <div className="text-xs text-muted-foreground">dotfile manager</div>
          </div>
        </div>

        <Separator />

        <nav className="flex flex-col gap-1 p-3">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => onViewChange(item.id)}
              className={cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                view === item.id
                  ? "bg-accent font-medium text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
              )}
            >
              <item.icon className="size-4" />
              {item.label}
            </button>
          ))}
        </nav>

        <div className="mt-auto p-4">
          <Separator className="mb-4" />
          {environment ? (
            <div className="space-y-2 text-xs">
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">System</span>
                <span className="truncate font-medium">{environment.distroName}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Family</span>
                <span className="truncate font-medium">{environment.familyLabel}</span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Symlinks</span>
                <Badge
                  variant={environment.symlinksSupported ? "success" : "destructive"}
                >
                  {environment.symlinksSupported ? "yes" : "no"}
                </Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Home writable</span>
                <Badge
                  variant={environment.homeWritable ? "success" : "destructive"}
                >
                  {environment.homeWritable ? "yes" : "no"}
                </Badge>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-muted-foreground">Git</span>
                {gitAvailable === null ? (
                  <span className="text-muted-foreground">…</span>
                ) : (
                  <Badge variant={gitAvailable ? "success" : "destructive"}>
                    {gitAvailable ? "available" : "missing"}
                  </Badge>
                )}
              </div>
              {environment.restricted && (
                <div className="mt-3 space-y-1.5 rounded-md border border-destructive/40 bg-destructive/10 p-2.5">
                  <div className="flex items-center gap-1.5 font-medium text-destructive">
                    <ShieldAlert className="size-3.5" />
                    Restricted system
                  </div>
                  {environment.restrictions.map((restriction) => (
                    <div key={restriction} className="text-muted-foreground">
                      {restriction}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="text-xs text-muted-foreground">Detecting system…</div>
          )}
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">{children}</main>
    </div>
  );
}
