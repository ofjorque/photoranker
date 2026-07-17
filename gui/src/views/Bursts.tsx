import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { PendingBurst, ResolvedBurst } from '@/api/types';
import { getProject } from '@/state';
import { showToast } from '@/toast';
import { getThumbnailDataUrl } from '@/api/thumbnailCache';
import { t } from '@/i18n';
import { RankingBoard } from '@/components/RankingBoard';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Layers, Undo2, Ban, Info } from 'lucide-react';
import { Html } from '@/components/Html';
import { PhotoDetailsDrawer } from '@/components/PhotoDetailsDrawer';

export function BurstsView() {
  const [project] = useState(getProject);
  const [pendingBurst, setPendingBurst] = useState<PendingBurst | null>(null);
  const [totalPending, setTotalPending] = useState(0);
  const [resolvedBursts, setResolvedBursts] = useState<ResolvedBurst[]>([]);
  const [showResolved, setShowResolved] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  
  // Exclude state
  const [excludedIds, setExcludedIds] = useState<Set<number>>(new Set());
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  const [detailsImageId, setDetailsImageId] = useState<number | null>(null);

  const loadData = useCallback(async () => {
    if (!project) return;
    setLoading(true);
    setError('');
    try {
      const bursts = await cli.listBursts(project.dbPath);
      setTotalPending(bursts.length);
      setPendingBurst(bursts.length > 0 ? bursts[0] : null);
      setExcludedIds(new Set());
      
      // Pre-load thumbnails for the exclude panel if > 2 images
      if (bursts.length > 0 && bursts[0].images.length > 2) {
        bursts[0].images.forEach(img => {
          getThumbnailDataUrl(project.dbPath, img.id).then(url => {
            if (url) setThumbnails(prev => ({ ...prev, [img.id]: url }));
          });
        });
      }
    } catch (e) {
      setError(e instanceof CliError ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [project]);

  const loadResolved = useCallback(async () => {
    if (!project) return;
    try {
      const resolved = await cli.listBurstsResolved(project.dbPath);
      setResolvedBursts(resolved.slice(0, 10)); // Top 10 as in original
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  }, [project]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  useEffect(() => {
    if (showResolved) {
      loadResolved();
    }
  }, [showResolved, loadResolved]);

  if (!project) {
    return <div className="p-6 text-center text-muted-foreground">{t('common.openProjectFirst')}</div>;
  }

  if (loading) {
    return <div className="p-6 text-center text-muted-foreground">{t('bursts.loadingPending')}</div>;
  }

  if (error) {
    return <div className="p-6 text-center text-destructive">{error}</div>;
  }

  if (!pendingBurst) {
    return (
      <div className="p-6 max-w-4xl mx-auto space-y-6">
        <h1 className="text-3xl font-bold tracking-tight">{t('bursts.title')}</h1>
        <Card className="bg-muted text-center p-12">
          <Layers className="w-12 h-12 mx-auto mb-4 text-muted-foreground opacity-50" />
          <Html className="text-xl font-medium" html={t('bursts.noPending')} />
        </Card>
      </div>
    );
  }

  const toggleExclude = (id: number, checked: boolean) => {
    setExcludedIds(prev => {
      const next = new Set(prev);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const handleExcludeSubmit = async () => {
    if (excludedIds.size === 0) {
      showToast(t('bursts.exclude.needSelection'), true);
      return;
    }
    try {
      const result = await cli.burstExclude(project.dbPath, pendingBurst.id, Array.from(excludedIds));
      showToast(
        result.burst_dissolved
          ? t('bursts.exclude.dissolved', { id: pendingBurst.id })
          : t('bursts.exclude.excluded', { count: result.excluded.length, id: pendingBurst.id })
      );
      await loadData();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const handleRankingSubmit = async (ranking: Array<[number, number]>) => {
    try {
      const result = await cli.burstTournament(project.dbPath, pendingBurst.id, ranking);
      showToast(
        t('bursts.tournamentResult', {
          id: pendingBurst.id,
          representativeId: result.representative_image_id,
          rejected: result.rejected,
        })
      );
      await loadData();
      if (showResolved) loadResolved();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const handleUndo = async (burstId: number) => {
    try {
      await cli.burstUndo(project.dbPath, burstId);
      showToast(t('bursts.resolved.undone', { id: burstId }));
      await loadResolved();
      await loadData();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="p-6 shrink-0 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">
            {t('bursts.heading', { id: pendingBurst.id })}
            <span className="text-muted-foreground font-normal ml-2 text-xl">
              ({totalPending} {totalPending === 1 ? t('bursts.pending.one') : t('bursts.pending.other')})
            </span>
          </h1>
          <Html className="text-muted-foreground mt-2 block" html={t('bursts.instructions')} />
        </div>
      </div>

      {pendingBurst.images.length > 2 && (
        <div className="px-6 shrink-0 pb-4">
          <Card>
            <CardHeader className="py-3 px-4">
              <CardTitle className="text-base">{t('bursts.exclude.question')}</CardTitle>
            </CardHeader>
            <CardContent className="px-4 pb-3 flex flex-col sm:flex-row items-end gap-4">
              <div className="flex-1 flex gap-2 overflow-x-auto pb-2">
                {pendingBurst.images.map(img => (
                  <label key={img.id} className="shrink-0 flex flex-col gap-2 cursor-pointer w-24">
                    <div className="relative aspect-[3/2] bg-muted overflow-hidden rounded border">
                      {thumbnails[img.id] && <img src={thumbnails[img.id]} className="w-full h-full object-cover" />}
                      <button
                        type="button"
                        title={t('photoDetails.viewDetails')}
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          setDetailsImageId(img.id);
                        }}
                        className="absolute top-1 right-1 w-5 h-5 flex items-center justify-center rounded-full bg-background/80 text-foreground hover:bg-background"
                      >
                        <Info className="w-3.5 h-3.5" />
                      </button>
                    </div>
                    <div className="flex items-center gap-2">
                      <Checkbox 
                        checked={excludedIds.has(img.id)} 
                        onCheckedChange={(c) => toggleExclude(img.id, !!c)} 
                      />
                      <span className="text-xs">{t('bursts.exclude.notBurst')}</span>
                    </div>
                  </label>
                ))}
              </div>
              <Button onClick={handleExcludeSubmit} variant="secondary" className="mb-2 shrink-0">
                <Ban className="w-4 h-4 mr-2" />
                {t('bursts.exclude.button')}
              </Button>
            </CardContent>
          </Card>
        </div>
      )}

      <div className="flex-1 min-h-0 relative border-t">
        <RankingBoard
          key={pendingBurst.id}
          dbPath={project.dbPath}
          images={pendingBurst.images}
          onSubmit={handleRankingSubmit}
        />
      </div>

      <div className="p-4 bg-muted/50 border-t shrink-0">
        <div 
          className="flex items-center gap-3 cursor-pointer select-none"
          onClick={() => setShowResolved(!showResolved)}
        >
          <h2 className="text-lg font-semibold">{t('bursts.resolved.title')}</h2>
          <span className="text-xs bg-muted text-muted-foreground px-2 py-1 rounded">
            {t('bursts.resolved.toggle')}
          </span>
        </div>
        
        {showResolved && (
          resolvedBursts.length === 0 ? (
            <p className="mt-4 text-sm text-muted-foreground">{t('bursts.resolved.empty')}</p>
          ) : (
            <ScrollArea className="mt-4 h-60">
              <div className="grid gap-2 pr-2">
                {resolvedBursts.map(burst => (
                  <div key={burst.id} className="flex items-center justify-between p-3 rounded-md border bg-card">
                    <span className="text-sm">
                      {t('bursts.resolved.row', { id: burst.id, representativeId: burst.representative_image_id ?? '?', count: burst.images.length })}
                    </span>
                    <Button variant="destructive" size="sm" onClick={() => handleUndo(burst.id)}>
                      <Undo2 className="w-3.5 h-3.5 mr-2" />
                      {t('bursts.resolved.undo')}
                    </Button>
                  </div>
                ))}
              </div>
            </ScrollArea>
          )
        )}
      </div>

      <PhotoDetailsDrawer
        dbPath={project.dbPath}
        imageId={detailsImageId}
        open={detailsImageId !== null}
        onOpenChange={(open) => !open && setDetailsImageId(null)}
      />
    </div>
  );
}
