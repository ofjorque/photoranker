import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { ClusterSummary } from '@/api/types';
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
          if (!thumbnails[img.id]) {
            const url = await getThumbnailDataUrl(project.dbPath, img.id);
            if (url) newThumbnails[img.id] = url;
          }
        }
      }
      setThumbnails(prev => ({ ...prev, ...newThumbnails }));
      setRenameInputs(prev => ({ ...newRenames, ...prev }));
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setListLoading(false);
    }
  }, [project, thumbnails]);

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
                    <span className="text-xs bg-muted text-muted-foreground px-2 py-1 rounded-full whitespace-nowrap">
                      {t('cluster.card.photoCount', { count: cluster.member_count })}
                    </span>
                  </div>
                  
                  <div 
                    className="grid gap-1 mb-4" 
                    style={{ gridTemplateColumns: `repeat(${cluster.representative_images.length || 1}, 1fr)` }}
                  >
                    {cluster.representative_images.map(img => (
                      <div key={img.id} className="relative aspect-square bg-muted rounded overflow-hidden">
                        {thumbnails[img.id] && (
                          <img
                            src={thumbnails[img.id]}
                            className="w-full h-full object-cover cursor-zoom-in"
                            onClick={() => openLightbox(thumbnails[img.id], img.file_path)}
                          />
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

      <PhotoDetailsDrawer
        dbPath={project.dbPath}
        imageId={detailsImageId}
        open={detailsImageId !== null}
        onOpenChange={(open) => !open && setDetailsImageId(null)}
      />
    </div>
  );
}
