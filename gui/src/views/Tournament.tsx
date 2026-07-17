import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { TournamentStatusResult, TournamentNextResult } from '@/api/types';
import { getProject } from '@/state';
import { showToast } from '@/toast';
import { t } from '@/i18n';
import { RankingBoard } from '@/components/RankingBoard';
import { QualityPanel } from '@/components/QualityPanel';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { ChevronRight, ChevronLeft, Filter, X } from 'lucide-react';
import { cn } from '@/lib/utils';

const QUALITY_PANEL_COLLAPSED_KEY = 'photoranker-quality-panel-collapsed';

export function TournamentView() {
  const [project] = useState(getProject);
  const [status, setStatus] = useState<TournamentStatusResult | null>(null);
  const [group, setGroup] = useState<TournamentNextResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [panelCollapsed, setPanelCollapsed] = useState(
    localStorage.getItem(QUALITY_PANEL_COLLAPSED_KEY) === 'true'
  );
  const [focusedImageId, setFocusedImageId] = useState<number | null>(null);

  // Scope por subcarpeta (ver docs/fase8-mejoras-avanzadas.md, "Acotar el
  // pool de torneo por subcarpeta") — `scopeInput` es lo que se está
  // escribiendo, `activeScope` lo que efectivamente se aplicó al último
  // tournament-next (no cambia mientras el usuario tipea, solo al confirmar).
  const [scopeInput, setScopeInput] = useState('');
  const [activeScope, setActiveScope] = useState<string | undefined>(undefined);

  const loadData = useCallback(async (scope?: string) => {
    if (!project) return;
    setLoading(true);
    try {
      const [statusRes, groupRes] = await Promise.all([
        cli.tournamentStatus(project.dbPath).catch(() => null),
        cli.tournamentNext(project.dbPath, scope).catch(e => {
          showToast(e instanceof CliError ? e.message : String(e), true);
          return null;
        })
      ]);
      setStatus(statusRes);
      setGroup(groupRes);
      setFocusedImageId(null);
    } finally {
      setLoading(false);
    }
  }, [project]);

  useEffect(() => {
    loadData(activeScope);
    // Solo al montar — cambios posteriores de scope se disparan
    // explícitamente desde handleApplyScope/handleClearScope, no acá.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadData]);

  const handleApplyScope = () => {
    const trimmed = scopeInput.trim();
    setActiveScope(trimmed || undefined);
    loadData(trimmed || undefined);
  };

  const handleClearScope = () => {
    setScopeInput('');
    setActiveScope(undefined);
    loadData(undefined);
  };

  const handlePanelToggle = () => {
    const next = !panelCollapsed;
    setPanelCollapsed(next);
    localStorage.setItem(QUALITY_PANEL_COLLAPSED_KEY, String(next));
  };

  const handleRankingSubmit = async (ranking: Array<[number, number]>) => {
    if (!project || !group) return;
    try {
      await cli.tournamentResult(project.dbPath, group.group_id, ranking);
      showToast(t('tournament.resultSent'));
      await loadData(activeScope);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  if (!project) {
    return <div className="p-6 text-center text-muted-foreground">{t('common.openProjectFirst')}</div>;
  }

  if (loading && !group) {
    return <div className="p-6 text-center text-muted-foreground">{t('tournament.loadingStatus')}</div>;
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="shrink-0 p-6 pb-2">
        {status ? (
          <div className="grid grid-cols-3 md:grid-cols-6 gap-4 mb-4 text-sm">
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.active')}</div>
              <div className="font-semibold text-lg">{status.active_images}</div>
            </div>
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.converged')}</div>
              <div className="font-semibold text-lg">{status.converged_images}</div>
            </div>
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.stalled')}</div>
              <div className="font-semibold text-lg">{status.stalled_images}</div>
            </div>
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.rounds')}</div>
              <div className="font-semibold text-lg">{status.rounds_completed}/{status.max_rounds}</div>
            </div>
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.convergencePct')}</div>
              <div className="font-semibold text-lg">{(status.convergence_ratio * 100).toFixed(0)}%</div>
            </div>
            <div className="bg-muted p-2 rounded text-center">
              <div className="text-muted-foreground">{t('tournament.status.state')}</div>
              <div className={cn("font-semibold text-lg", status.status === 'converged' ? "text-success" : "")}>
                {status.status}
              </div>
            </div>
          </div>
        ) : (
          <p className="text-destructive text-sm mb-4">{t('tournament.status.loadError')}</p>
        )}

        <div className="flex items-end gap-2 mb-4">
          <div className="space-y-1">
            <Label htmlFor="tournament-scope-input" className="text-xs text-muted-foreground flex items-center gap-1">
              <Filter className="w-3 h-3" />
              {t('tournament.scope.label')}
            </Label>
            <Input
              id="tournament-scope-input"
              value={scopeInput}
              onChange={(e) => setScopeInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleApplyScope()}
              placeholder={t('tournament.scope.placeholder')}
              className="h-8 w-56"
            />
          </div>
          <Button variant="secondary" size="sm" onClick={handleApplyScope} disabled={loading}>
            {t('tournament.scope.apply')}
          </Button>
          {activeScope && (
            <Button variant="ghost" size="sm" onClick={handleClearScope} disabled={loading}>
              <X className="w-3.5 h-3.5 mr-1" />
              {t('tournament.scope.clear', { scope: activeScope })}
            </Button>
          )}
        </div>

        {group ? (
          <h1 className="text-2xl font-bold">
            {t('tournament.groupHeading', { groupId: group.group_id.slice(0, 8) })}
          </h1>
        ) : (
          <div className="text-center text-muted-foreground py-12">{t('tournament.noGroup')}</div>
        )}
      </div>

      {group && (
        <div className="flex-1 min-h-0 flex overflow-hidden">
          <div className="flex-1 min-w-0 border-t border-r relative">
            <RankingBoard
              key={group.group_id}
              dbPath={project.dbPath}
              images={group.images}
              captionFor={(img) => {
                const found = group.images.find(i => i.id === img.id);
                return found ? `μ=${found.mu.toFixed(1)} σ=${found.sigma.toFixed(1)}` : '';
              }}
              onFocusChange={(img) => setFocusedImageId(img.id)}
              onSubmit={handleRankingSubmit}
            />
          </div>
          
          <div className={cn(
            "bg-muted/30 flex flex-col transition-all duration-300 overflow-hidden",
            panelCollapsed ? "w-10" : "w-80"
          )}>
            <div className="shrink-0 p-2 border-b flex items-center justify-between">
              {!panelCollapsed && <h3 className="font-semibold text-sm px-2">{t('tournament.quality.title')}</h3>}
              <Button variant="ghost" size="icon" onClick={handlePanelToggle} className="h-6 w-6">
                {panelCollapsed ? <ChevronLeft className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
              </Button>
            </div>
            {!panelCollapsed && (
              <div className="flex-1 overflow-auto p-4">
                <QualityPanel dbPath={project.dbPath} imageId={focusedImageId} />
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
