import { useEffect, useState } from "react";

import { AppShell, type View } from "@/components/app-shell";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { DotfilesPage } from "@/pages/dotfiles";
import { SnapshotsPage } from "@/pages/snapshots";
import { api } from "@/lib/api";
import type { EnvironmentSummary } from "@/lib/types";

export default function App() {
  const [view, setView] = useState<View>("dotfiles");
  const [environment, setEnvironment] = useState<EnvironmentSummary | null>(null);
  const [gitAvailable, setGitAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    api.environmentSummary().then(setEnvironment).catch(() => undefined);
    api.checkGitAvailable().then(setGitAvailable).catch(() => undefined);
  }, []);

  return (
    <TooltipProvider>
      <AppShell
        view={view}
        onViewChange={setView}
        environment={environment}
        gitAvailable={gitAvailable}
      >
        {view === "dotfiles" ? <DotfilesPage /> : <SnapshotsPage />}
      </AppShell>
      <Toaster />
    </TooltipProvider>
  );
}
