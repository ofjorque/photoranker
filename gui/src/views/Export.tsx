import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { RankingEntry, FailedThumbnail } from '@/api/types';
import { getProject } from '@/state';
import { showToast } from '@/toast';
import { t } from '@/i18n';
import { Button } from '@/components/ui/button';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from '@/components/ui/card';
import { Loader2, RefreshCw, Info } from 'lucide-react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { ScrollArea } from '@/components/ui/scroll-area';
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu';
import { PhotoDetailsDrawer } from '@/components/PhotoDetailsDrawer';

export function ExportView() {
  const [project] = useState(getProject);
  
  const [ranking, setRanking] = useState<RankingEntry[]>([]);
  const [rankingLoading, setRankingLoading] = useState(true);
  
  const [failed, setFailed] = useState<FailedThumbnail[]>([]);
  const [failedLoading, setFailedLoading] = useState(true);
  
  const [exportBusy, setExportBusy] = useState(false);
  const [exportResult, setExportResult] = useState<any>(null);
  
  // Custom Loading Dialog for Export (replaces LoadingOverlay)
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [exportDialogStatus, setExportDialogStatus] = useState<'loading' | 'done'>('loading');

  const [detailsImageId, setDetailsImageId] = useState<number | null>(null);

  const loadRanking = useCallback(async () => {
    if (!project) return;
    setRankingLoading(true);
    try {
      const rows = await cli.ranking(project.dbPath);
      setRanking(rows);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setRankingLoading(false);
    }
  }, [project]);

  const loadFailed = useCallback(async () => {
    if (!project) return;
    setFailedLoading(true);
    try {
      const rows = await cli.listFailedThumbnails(project.dbPath);
      setFailed(rows);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setFailedLoading(false);
    }
  }, [project]);

  useEffect(() => {
    loadRanking();
    loadFailed();
  }, [loadRanking, loadFailed]);

  const handleRetryThumbnail = async (id: number) => {
    if (!project) return;
    try {
      await cli.retryThumbnail(project.dbPath, id);
      showToast(t('export.failedThumbnails.retried', { id }));
      await loadFailed();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const handleExport = async () => {
    if (!project) return;
    setShowExportDialog(true);
    setExportDialogStatus('loading');
    setExportBusy(true);
    setExportResult(null);
    try {
      const data = await cli.exportXmp(project.dbPath);
      setExportResult(data);
      setExportDialogStatus('done');
    } catch (e) {
      setShowExportDialog(false);
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setExportBusy(false);
    }
  };

  if (!project) {
    return <div className="p-6 text-center text-muted-foreground">{t('common.openProjectFirst')}</div>;
  }

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold tracking-tight">{t('export.title')}</h1>

      <Tabs defaultValue="ranking">
        <TabsList>
          <TabsTrigger value="ranking">{t('export.tabs.ranking')}</TabsTrigger>
          <TabsTrigger value="failed">{t('export.tabs.failed')}</TabsTrigger>
          <TabsTrigger value="xmp">{t('export.tabs.xmp')}</TabsTrigger>
        </TabsList>

      <TabsContent value="ranking">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-lg">{t('export.liveRanking.title')}</CardTitle>
          <Button variant="ghost" size="sm" onClick={loadRanking} disabled={rankingLoading}>
            <RefreshCw className={`w-4 h-4 mr-2 ${rankingLoading ? 'animate-spin' : ''}`} />
            {t('common.refresh')}
          </Button>
        </CardHeader>
        <CardContent>
          {rankingLoading && ranking.length === 0 ? (
            <div className="text-muted-foreground text-sm">{t('common.loading')}</div>
          ) : ranking.length === 0 ? (
            <div className="text-muted-foreground text-sm">{t('export.liveRanking.empty')}</div>
          ) : (
            <ScrollArea className="h-80 rounded-md border">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted sticky top-0">
                  <tr>
                    <th className="px-4 py-2 font-medium">#</th>
                    <th className="px-4 py-2 font-medium">{t('export.liveRanking.colFile')}</th>
                    <th className="px-4 py-2 font-medium">μ</th>
                    <th className="px-4 py-2 font-medium">σ</th>
                    <th className="px-4 py-2 font-medium">{t('export.liveRanking.colState')}</th>
                    <th className="px-4 py-2 font-medium"></th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {ranking.map((r, i) => (
                    <ContextMenu key={r.file_path}>
                      <ContextMenuTrigger asChild>
                        <tr className="hover:bg-muted/50">
                          <td className="px-4 py-2 font-mono text-muted-foreground">{i + 1}</td>
                          <td className="px-4 py-2 truncate max-w-[200px]" title={r.file_path}>
                            {r.file_path.split(/[\\/]/).pop()}
                          </td>
                          <td className="px-4 py-2 font-mono">{r.mu.toFixed(2)}</td>
                          <td className="px-4 py-2 font-mono">{r.sigma.toFixed(2)}</td>
                          <td className="px-4 py-2">
                            {r.rejected ? (
                              <span className="text-xs bg-destructive/10 text-destructive px-2 py-1 rounded">rejected</span>
                            ) : r.stalled ? (
                              <span className="text-xs bg-muted text-muted-foreground px-2 py-1 rounded">stalled</span>
                            ) : (
                              <span className="text-xs bg-green-500/10 text-green-600 px-2 py-1 rounded">{t('export.liveRanking.active')}</span>
                            )}
                          </td>
                          <td className="px-4 py-2 text-right">
                            <Button
                              variant="ghost"
                              size="icon"
                              title={t('photoDetails.viewDetails')}
                              onClick={() => setDetailsImageId(r.id)}
                            >
                              <Info className="w-4 h-4" />
                            </Button>
                          </td>
                        </tr>
                      </ContextMenuTrigger>
                      <ContextMenuContent>
                        <ContextMenuItem onClick={() => setDetailsImageId(r.id)}>
                          {t('photoDetails.viewDetails')}
                        </ContextMenuItem>
                      </ContextMenuContent>
                    </ContextMenu>
                  ))}
                </tbody>
              </table>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="failed">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-lg">{t('export.failedThumbnails.title')}</CardTitle>
          <Button variant="ghost" size="sm" onClick={loadFailed} disabled={failedLoading}>
            <RefreshCw className={`w-4 h-4 mr-2 ${failedLoading ? 'animate-spin' : ''}`} />
            {t('common.refresh')}
          </Button>
        </CardHeader>
        <CardContent>
          {failedLoading && failed.length === 0 ? (
            <div className="text-muted-foreground text-sm">{t('common.loading')}</div>
          ) : failed.length === 0 ? (
            <div className="text-muted-foreground text-sm">{t('export.failedThumbnails.empty')}</div>
          ) : (
            <ScrollArea className="h-60 rounded-md border">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted sticky top-0">
                  <tr>
                    <th className="px-4 py-2 font-medium">ID</th>
                    <th className="px-4 py-2 font-medium">{t('export.liveRanking.colFile')}</th>
                    <th className="px-4 py-2 font-medium"></th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {failed.map(r => (
                    <tr key={r.id} className="hover:bg-muted/50">
                      <td className="px-4 py-2 font-mono text-muted-foreground">{r.id}</td>
                      <td className="px-4 py-2 truncate max-w-[200px]" title={r.file_path}>
                        {r.file_path.split(/[\\/]/).pop()}
                      </td>
                      <td className="px-4 py-2 text-right">
                        <Button variant="outline" size="sm" onClick={() => handleRetryThumbnail(r.id)}>
                          retry-thumbnail
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="xmp">
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('export.xmp.title')}</CardTitle>
          <CardDescription>{t('export.xmp.description')}</CardDescription>
        </CardHeader>
        <CardContent>
          <Button onClick={handleExport} disabled={exportBusy}>
            export-xmp
          </Button>
          {exportResult && (
            <ScrollArea className="mt-4 h-60 rounded-md border">
              <pre className="p-4 bg-muted text-xs font-mono">
                {JSON.stringify(exportResult, null, 2)}
              </pre>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
      </TabsContent>
      </Tabs>

      <Dialog open={showExportDialog} onOpenChange={setShowExportDialog}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>
              {exportDialogStatus === 'loading' ? t('export.xmp.loadingTitle') : t('loadingOverlay.done')}
            </DialogTitle>
            <DialogDescription>
              {exportDialogStatus === 'loading' && (
                <div className="flex items-center mt-4">
                  <Loader2 className="w-5 h-5 mr-3 animate-spin text-primary" />
                  <span>{t('export.phase.writing')}</span>
                </div>
              )}
              {exportDialogStatus === 'done' && exportResult && (
                <div className="mt-4 text-foreground">
                  {t('export.xmp.result', {
                    written: exportResult.written,
                    failedThumbnail: exportResult.excluded_failed_thumbnail,
                    missing: exportResult.excluded_missing,
                  })}
                </div>
              )}
            </DialogDescription>
          </DialogHeader>
          {exportDialogStatus === 'done' && (
            <div className="flex justify-end mt-4">
              <Button onClick={() => setShowExportDialog(false)}>{t('common.close')}</Button>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <PhotoDetailsDrawer
        dbPath={project.dbPath}
        imageId={detailsImageId}
        open={detailsImageId !== null}
        onOpenChange={(open) => !open && setDetailsImageId(null)}
      />
    </div>
  );
}
