import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { ClusterRepresentativeImage, ClusterSummary } from '@/api/types';
import { getProject } from '@/state';
import { showToast } from '@/toast';
import { getThumbnailDataUrl } from '@/api/thumbnailCache';
import { t } from '@/i18n';
import { ScreePlot } from '@/components/ScreePlot';
import { Button } from '@/components/ui/button';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Slider } from '@/components/ui/slider';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import { Progress } from '@/components/ui/progress';
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from '@/components/ui/sheet';
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu';
import { Loader2, Info } from 'lucide-react';
import { Html } from '@/components/Html';
import { openLightbox } from '@/components/Lightbox';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { PhotoDetailsDrawer } from '@/components/PhotoDetailsDrawer';

export function ClusterView() {
  const [project] = useState(getProject);
  
  // List state
  const [clusters, setClusters] = useState<ClusterSummary[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  const [renameInputs, setRenameInputs] = useState<Record<string, string>>({});
  const [detailsImageId, setDetailsImageId] = useState<number | null>(null);

  // "Ver todas" (todas las imágenes de un cluster, no solo las
  // representativas) — ver docs/fase7-mejoras-post-mvp.md.
  const [viewingCluster, setViewingCluster] = useState<ClusterSummary | null>(null);
  const [clusterImages, setClusterImages] = useState<ClusterRepresentativeImage[]>([]);
  const [clusterImagesLoading, setClusterImagesLoading] = useState(false);

  // Preview state
  const [previewBusy, setPreviewBusy] = useState(false);
  const [bicByK, setBicByK] = useState<Record<string, number> | null>(null);
  const [previewError, setPreviewError] = useState('');
  
  // Commit state
  const [commitBusy, setCommitBusy] = useState(false);
  const [kInput, setKInput] = useState('');
  const [probThreshold, setProbThreshold] = useState(0);
  const [commitResult, setCommitResult] = useState<any>(null);

  const loadClusters = useCallback(async () => {
    if (!project) return;
    setListLoading(true);
    try {
      const data = await cli.listClusters(project.dbPath);
      setClusters(data);
      
      const newThumbnails: Record<number, string> = {};
      const newRenames: Record<string, string> = {};
      for (const cluster of data) {
        newRenames[cluster.id] = cluster.name ?? '';
        for (const img of cluster.representative_images) {
          // getThumbnailDataUrl ya cachea en memoria por (dbPath, imageId) —
          // no hace falta chequear `thumbnails` acá (y no conviene: si esta
          // función dependiera de `thumbnails`, cada `setThumbnails` de más
          // abajo recrearía `loadClusters`, lo que retrigger-ea el efecto que
          // la llama y produce un loop de refetch infinito — la causa real
          // del flicker reportado en la grilla de clusters).
          const url = await getThumbnailDataUrl(project.dbPath, img.id);
          if (url) newThumbnails[img.id] = url;
        }
      }
      setThumbnails(prev => ({ ...prev, ...newThumbnails }));
      setRenameInputs(prev => ({ ...newRenames, ...prev }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setListLoading(false);
    }
  }, [project]);

  useEffect(() => {
    loadClusters();
  }, [loadClusters]);

  const handleRename = async (id: number) => {
    if (!project) return;
    const name = renameInputs[id]?.trim();
    if (!name) {
      showToast(t('cluster.card.nameRequired'), true);
      return;
    }
    try {
      await cli.clusterRename(project.dbPath, id, name);
      showToast(t('cluster.card.renamed', { id, name }));
      await loadClusters();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const openClusterImages = async (cluster: ClusterSummary) => {
    if (!project) return;
    setViewingCluster(cluster);
    setClusterImages([]);
    setClusterImagesLoading(true);
    try {
      const images = await cli.listClusterImages(project.dbPath, cluster.id);
      setClusterImages(images);
      for (const img of images) {
        if (thumbnails[img.id]) continue;
        const url = await getThumbnailDataUrl(project.dbPath, img.id);
        if (url) setThumbnails(prev => ({ ...prev, [img.id]: url }));
      }
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setClusterImagesLoading(false);
    }
  };

  const handlePreview = async () => {
    if (!project) return;
    setPreviewBusy(true);
    setPreviewError('');
    try {
      const data = await cli.clusterPreview(project.dbPath);
      setBicByK(data.bic_by_k);
    } catch (e) {
      setPreviewError(e instanceof CliError ? e.message : String(e));
    } finally {
      setPreviewBusy(false);
    }
  };

  const handleCommit = async () => {
    if (!project) return;
    setCommitBusy(true);
    setCommitResult(null);
    try {
      const k = kInput.trim() === '' ? undefined : Number(kInput);
      const data = await cli.clusterCommit(project.dbPath, k, probThreshold > 0 ? probThreshold : undefined);
      setCommitResult(data);
      showToast(data.from_cache ? t('cluster.commit.doneFromCache') : t('cluster.commit.done'));
      await loadClusters();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setCommitBusy(false);
    }
  };

  if (!project) {
    return <div className="p-6 text-center text-muted-foreground">{t('common.openProjectFirst')}</div>;
  }

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold tracking-tight">{t('cluster.title')}</h1>

      <Tabs defaultValue="preview">
        <TabsList>
          <TabsTrigger value="preview">{t('cluster.tabs.preview')}</TabsTrigger>
          <TabsTrigger value="commit">{t('cluster.tabs.commit')}</TabsTrigger>
          <TabsTrigger value="list">{t('cluster.tabs.list')}</TabsTrigger>
        </TabsList>

      <TabsContent value="preview">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle className="text-lg">{t('cluster.preview.title')}</CardTitle>
          <Button onClick={handlePreview} disabled={previewBusy} variant="secondary">
            {previewBusy && <Loader2 className="w-4 h-4 mr-2 animate-spin" />}
            cluster --preview
          </Button>
        </CardHeader>
        <CardContent>
          {previewError && <div className="text-destructive mb-4">{previewError}</div>}
          {bicByK && <ScreePlot bicByK={bicByK} />}
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="commit">
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('cluster.commit.title')}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-end gap-4">
            <div className="space-y-1">
              <Label htmlFor="cluster-k-input">{t('cluster.commit.kLabel')}</Label>
              <Input
                id="cluster-k-input"
                type="number"
                min={2}
                max={10}
                value={kInput}
                onChange={e => setKInput(e.target.value)}
                className="w-32"
              />
            </div>
            <Button onClick={handleCommit} disabled={commitBusy}>
              {commitBusy && <Loader2 className="w-4 h-4 mr-2 animate-spin" />}
              cluster --k
            </Button>
          </div>

          {bicByK && (
            <div className="space-y-2 p-4 bg-muted/50 rounded-md border mt-4">
              <div className="flex justify-between">
                <Label htmlFor="cluster-prob-threshold">{t('cluster.probThreshold.label')}</Label>
                <span className="text-sm font-mono">{probThreshold.toFixed(2)}</span>
              </div>
              <Slider
                id="cluster-prob-threshold"
                min={0}
                max={1}
                step={0.05}
                value={[probThreshold]}
                onValueChange={([v]) => setProbThreshold(v)}
              />
            </div>
          )}

          {commitResult && (
            <ScrollArea className="mt-4 h-60 rounded-md border">
              <pre className="p-4 bg-muted text-xs font-mono">
                {JSON.stringify(commitResult, null, 2)}
              </pre>
            </ScrollArea>
          )}
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="list">
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('cluster.list.title')}</CardTitle>
        </CardHeader>
        <CardContent>
          {listLoading ? (
            <div className="text-muted-foreground">{t('cluster.list.loading')}</div>
          ) : clusters.length === 0 ? (
            <Html className="text-muted-foreground" html={t('cluster.list.empty')} />
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {clusters.map(cluster => (
                <div key={cluster.id} className="border rounded-md p-4 bg-card shadow-sm">
                  <div className="flex justify-between items-center mb-3">
                    <strong className="truncate font-medium" title={cluster.name || String(cluster.id)}>
                      {cluster.name ?? t('cluster.card.unnamed', { id: String(cluster.id).substring(0, 6) })}
                    </strong>
                    <button
                      type="button"
                      onClick={() => openClusterImages(cluster)}
                      className="text-xs bg-muted text-muted-foreground hover:text-foreground px-2 py-1 rounded-full whitespace-nowrap transition-colors"
                    >
                      {t('cluster.card.viewAll', { count: cluster.member_count })}
                    </button>
                  </div>

                  <div
                    className="grid gap-1 mb-4"
                    style={{ gridTemplateColumns: `repeat(${cluster.representative_images.length || 1}, 1fr)` }}
                  >
                    {cluster.representative_images.map(img => (
                      <ContextMenu key={img.id}>
                        <ContextMenuTrigger asChild>
                          <div className="relative aspect-square bg-muted rounded overflow-hidden">
                            {thumbnails[img.id] ? (
                              <img
                                src={thumbnails[img.id]}
                                className="w-full h-full object-cover cursor-zoom-in"
                                onClick={() => openLightbox(thumbnails[img.id], img.file_path, { dbPath: project.dbPath, imageId: img.id })}
                              />
                            ) : (
                              <Skeleton className="w-full h-full rounded-none" />
                            )}
                            <button
                              type="button"
                              title={t('photoDetails.viewDetails')}
                              onClick={(e) => {
                                e.stopPropagation();
                                setDetailsImageId(img.id);
                              }}
                              className="absolute top-1 right-1 w-5 h-5 flex items-center justify-center rounded-full bg-background/80 text-foreground hover:bg-background"
                            >
                              <Info className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </ContextMenuTrigger>
                        <ContextMenuContent>
                          <ContextMenuItem
                            disabled={!thumbnails[img.id]}
                            onClick={() => openLightbox(thumbnails[img.id], img.file_path, { dbPath: project.dbPath, imageId: img.id })}
                          >
                            {t('rankingBoard.contextMenu.viewLarge')}
                          </ContextMenuItem>
                          <ContextMenuItem onClick={() => setDetailsImageId(img.id)}>
                            {t('photoDetails.viewDetails')}
                          </ContextMenuItem>
                        </ContextMenuContent>
                      </ContextMenu>
                    ))}
                  </div>

                  <div className="flex gap-2">
                    <Input 
                      placeholder={t('cluster.card.namePlaceholder')}
                      value={renameInputs[cluster.id] ?? ''}
                      onChange={e => setRenameInputs(prev => ({ ...prev, [cluster.id]: e.target.value }))}
                    />
                    <Button variant="secondary" onClick={() => handleRename(cluster.id)}>
                      {t('cluster.card.rename')}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
      </TabsContent>
      </Tabs>

      <Sheet open={viewingCluster !== null} onOpenChange={(open) => !open && setViewingCluster(null)}>
        <SheetContent className="w-full sm:max-w-md flex flex-col">
          <SheetHeader>
            <SheetTitle>
              {t('cluster.allImages.title', {
                name: viewingCluster?.name ?? t('cluster.card.unnamed', { id: String(viewingCluster?.id ?? '').substring(0, 6) }),
              })}
            </SheetTitle>
            <SheetDescription>{t('cluster.allImages.description')}</SheetDescription>
          </SheetHeader>
          {clusterImagesLoading ? (
            <div className="text-muted-foreground text-sm mt-4">{t('cluster.allImages.loading')}</div>
          ) : (
            <ScrollArea className="flex-1 -mx-6 px-6">
              <div className="space-y-3 pb-4">
                {clusterImages.map(img => (
                  <ContextMenu key={img.id}>
                    <ContextMenuTrigger asChild>
                      <div className="flex items-center gap-3">
                        <div className="relative w-14 h-14 shrink-0 rounded overflow-hidden bg-muted">
                          {thumbnails[img.id] ? (
                            <img
                              src={thumbnails[img.id]}
                              className="w-full h-full object-cover cursor-zoom-in"
                              onClick={() => openLightbox(thumbnails[img.id], img.file_path, { dbPath: project.dbPath, imageId: img.id })}
                            />
                          ) : (
                            <Skeleton className="w-full h-full rounded-none" />
                          )}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="truncate text-sm" title={img.file_path}>
                            {img.file_path.split(/[\\/]/).pop()}
                          </div>
                          <div className="flex items-center gap-2 mt-1">
                            <Progress value={(img.probability ?? 0) * 100} className="h-1.5" />
                            <span className="text-xs font-mono text-muted-foreground w-10 text-right shrink-0">
                              {((img.probability ?? 0) * 100).toFixed(0)}%
                            </span>
                          </div>
                        </div>
                        <Button
                          variant="ghost"
                          size="icon"
                          title={t('photoDetails.viewDetails')}
                          onClick={() => setDetailsImageId(img.id)}
                        >
                          <Info className="w-4 h-4" />
                        </Button>
                      </div>
                    </ContextMenuTrigger>
                    <ContextMenuContent>
                      <ContextMenuItem
                        disabled={!thumbnails[img.id]}
                        onClick={() => openLightbox(thumbnails[img.id], img.file_path, { dbPath: project.dbPath, imageId: img.id })}
                      >
                        {t('rankingBoard.contextMenu.viewLarge')}
                      </ContextMenuItem>
                      <ContextMenuItem onClick={() => setDetailsImageId(img.id)}>
                        {t('photoDetails.viewDetails')}
                      </ContextMenuItem>
                    </ContextMenuContent>
                  </ContextMenu>
                ))}
              </div>
            </ScrollArea>
          )}
        </SheetContent>
      </Sheet>

      <PhotoDetailsDrawer
        dbPath={project.dbPath}
        imageId={detailsImageId}
        open={detailsImageId !== null}
        onOpenChange={(open) => !open && setDetailsImageId(null)}
      />
    </div>
  );
}
