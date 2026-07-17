import { useState, useEffect, useCallback } from 'react';
import { cli, CliError } from '@/api';
import type { UserVariable, VariableValueEntry } from '@/api/types';
import { getProject } from '@/state';
import { showToast } from '@/toast';
import { getThumbnailDataUrl } from '@/api/thumbnailCache';
import { confirmDialog } from '@/components/ConfirmDialog';
import { isTypingTarget } from '@/utils/dom';
import { t } from '@/i18n';
import { Button } from '@/components/ui/button';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { Trash2, Info } from 'lucide-react';
import { Html } from '@/components/Html';
import { openLightbox } from '@/components/Lightbox';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { VariableBuilder, type VariableBuilderInput } from '@/components/VariableBuilder';
import { PhotoDetailsDrawer } from '@/components/PhotoDetailsDrawer';

interface VariableCardImage {
  id: number;
  file_path: string;
  values: Record<string, number | null>;
}

export function VariablesView() {
  const [project] = useState(getProject);
  const [variables, setVariables] = useState<UserVariable[]>([]);
  const [loading, setLoading] = useState(true);

  const [createBusy, setCreateBusy] = useState(false);

  // Classifier state
  const [classifyVar, setClassifyVar] = useState('');
  const [classifyEntries, setClassifyEntries] = useState<VariableValueEntry[]>([]);
  const [classifyIndex, setClassifyIndex] = useState(-1);
  const [classifyThumb, setClassifyThumb] = useState<string>('');

  // Vista de tarjetas (fotos × todas sus variables) — ver
  // docs/fase7-mejoras-post-mvp.md.
  const [cardImages, setCardImages] = useState<VariableCardImage[]>([]);
  const [cardsLoading, setCardsLoading] = useState(false);
  const [cardThumbnails, setCardThumbnails] = useState<Record<number, string>>({});
  const [cardsDetailsImageId, setCardsDetailsImageId] = useState<number | null>(null);

  const loadVariables = useCallback(async () => {
    if (!project) return;
    try {
      const vars = await cli.variableList(project.dbPath);
      setVariables(vars);
      if (vars.length > 0) {
        if (!classifyVar || !vars.find(v => v.name === classifyVar)) setClassifyVar(vars[0].name);
      } else {
        setClassifyVar('');
      }
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setLoading(false);
    }
  }, [project, classifyVar]);

  useEffect(() => {
    loadVariables();
  }, [loadVariables]);

  const handleDelete = async (name: string) => {
    if (!project) return;
    const confirmed = await confirmDialog({
      title: t('variables.list.deleteDialogTitle'),
      message: t('variables.list.deleteDialogMessage', { name }),
      confirmLabel: t('variables.list.deleteDialogConfirm'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      const result = await cli.variableDelete(project.dbPath, name);
      showToast(t('variables.list.deleted', { name, count: result.values_deleted }));
      await loadVariables();
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const handleCreate = async (inputs: VariableBuilderInput[]) => {
    if (!project) return;
    setCreateBusy(true);
    let created = 0;
    try {
      for (const input of inputs) {
        try {
          await cli.variableCreate(project.dbPath, input.name, input.varType, {
            min: input.min,
            max: input.max,
            categories: input.categories,
          });
          created += 1;
        } catch (e) {
          showToast(`${input.name}: ${e instanceof CliError ? e.message : String(e)}`, true);
        }
      }
      if (created > 0) {
        showToast(
          inputs.length === 1
            ? t('variables.create.created', { name: inputs[0].name })
            : t('variables.create.createdMany', { count: created }),
        );
      }
      await loadVariables();
    } finally {
      setCreateBusy(false);
    }
  };

  const loadCards = useCallback(async () => {
    if (!project || variables.length === 0) return;
    setCardsLoading(true);
    try {
      const byImage = new Map<number, VariableCardImage>();
      for (const v of variables) {
        const entries = await cli.getVariableValues(project.dbPath, v.name);
        for (const entry of entries) {
          let card = byImage.get(entry.id);
          if (!card) {
            card = { id: entry.id, file_path: entry.file_path, values: {} };
            byImage.set(entry.id, card);
          }
          card.values[v.name] = entry.value;
        }
      }
      const images = Array.from(byImage.values()).sort((a, b) => a.id - b.id);
      setCardImages(images);
      for (const img of images) {
        if (cardThumbnails[img.id]) continue;
        const url = await getThumbnailDataUrl(project.dbPath, img.id);
        if (url) setCardThumbnails(prev => ({ ...prev, [img.id]: url }));
      }
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setCardsLoading(false);
    }
    // cardThumbnails deliberadamente fuera de deps (mismo motivo que
    // Cluster.tsx): agregarla recrearía este callback en cada thumbnail
    // cargado y produciría un loop de refetch.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project, variables]);

  const startClassify = async () => {
    if (!project || !classifyVar) {
      showToast(t('variables.classify.needVariable'), true);
      return;
    }
    try {
      const entries = await cli.getVariableValues(project.dbPath, classifyVar);
      if (entries.length === 0) {
        showToast(t('variables.classify.noActiveImages'), true);
        return;
      }
      setClassifyEntries(entries);
      setClassifyIndex(0);
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    }
  };

  const closeClassify = () => {
    setClassifyIndex(-1);
    setClassifyEntries([]);
  };

  useEffect(() => {
    if (classifyIndex >= 0 && classifyIndex < classifyEntries.length) {
      const entry = classifyEntries[classifyIndex];
      setClassifyThumb(''); // clear
      getThumbnailDataUrl(project!.dbPath, entry.id).then(url => {
        if (url) setClassifyThumb(url);
      });
    }
  }, [classifyIndex, classifyEntries, project]);

  // Classifier keyboard nav
  useEffect(() => {
    if (classifyIndex < 0) return;
    
    const activeVar = variables.find(v => v.name === classifyVar);
    if (!activeVar) return;

    const validCodes = () => {
      if (activeVar.var_type === 'nominal') return activeVar.categories.map(c => c.code);
      const min = activeVar.min_value ?? 1;
      const max = activeVar.max_value ?? 5;
      const codes: number[] = [];
      for (let v = min; v <= max; v++) codes.push(v);
      return codes;
    };

    const goNext = () => setClassifyIndex(i => (i < classifyEntries.length - 1 ? i + 1 : i));
    const goPrev = () => setClassifyIndex(i => (i > 0 ? i - 1 : i));
    const assign = async (code: number) => {
      const entry = classifyEntries[classifyIndex];
      try {
        await cli.variableSet(project!.dbPath, activeVar.name, [[entry.id, code]]);
        setClassifyEntries(prev => {
          const next = [...prev];
          next[classifyIndex] = { ...next[classifyIndex], value: code };
          return next;
        });
        showToast(t('variables.classifier.assigned', { 
          file: entry.file_path.split(/[\\/]/).pop() ?? entry.file_path, 
          label: getOptionLabel(activeVar, code)
        }));
        goNext();
      } catch (e) {
        showToast(e instanceof CliError ? e.message : String(e), true);
      }
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target as Element)) return;
      if (e.key === 'Escape') {
        e.preventDefault();
        closeClassify();
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        goPrev();
      } else if (e.key === 'ArrowRight' || e.key === ' ') {
        e.preventDefault();
        goNext();
      } else if (e.key === 'Backspace') {
        e.preventDefault();
        goPrev();
      } else {
        const n = Number(e.key);
        if (Number.isInteger(n) && validCodes().includes(n)) {
          e.preventDefault();
          assign(n);
        }
      }
    };

    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [classifyIndex, classifyEntries, classifyVar, variables, project]);

  if (!project) return <div className="p-6 text-center text-muted-foreground">{t('common.openProjectFirst')}</div>;
  if (loading) return <div className="p-6 text-center text-muted-foreground">{t('common.loading')}</div>;

  const activeVar = variables.find(v => v.name === classifyVar);

  function getOptionLabel(v: UserVariable, code: number) {
    if (v.var_type === 'nominal') {
      const cat = v.categories.find(c => c.code === code);
      return cat ? `${code} = ${cat.label}` : String(code);
    }
    return String(code);
  }

  function getValidCodes(v: UserVariable) {
    if (v.var_type === 'nominal') return v.categories.map(c => c.code);
    const min = v.min_value ?? 1;
    const max = v.max_value ?? 5;
    const codes: number[] = [];
    for (let i = min; i <= max; i++) codes.push(i);
    return codes;
  }

  if (classifyIndex >= 0 && activeVar) {
    const entry = classifyEntries[classifyIndex];
    const codes = getValidCodes(activeVar);
    const name = entry.file_path.split(/[\\/]/).pop() ?? entry.file_path;
    
    return (
      <div className="flex flex-col items-center justify-center h-full p-6 bg-background">
        <Card className="w-full max-w-md">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <span className="font-mono text-sm">{classifyIndex + 1} / {classifyEntries.length}</span>
              <span className="text-xs bg-muted px-2 py-1 rounded">
                {entry.value == null 
                  ? t('variables.classifier.unassigned') 
                  : t('variables.classifier.valueLabel', { label: getOptionLabel(activeVar, entry.value) })
                }
              </span>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="aspect-[4/3] bg-muted rounded overflow-hidden flex items-center justify-center">
              {classifyThumb ? (
                <img
                  src={classifyThumb}
                  alt={name}
                  className="w-full h-full object-cover cursor-zoom-in"
                  onClick={() => openLightbox(classifyThumb, name)}
                />
              ) : (
                <span className="text-muted-foreground text-sm">{t('common.loading')}</span>
              )}
            </div>
            <div className="text-sm text-muted-foreground break-all">{name}</div>
            
            <div className="flex flex-wrap gap-2">
              {codes.map(code => (
                <Button 
                  key={code} 
                  variant={entry.value === code ? "default" : "secondary"}
                  onClick={() => {
                    cli.variableSet(project.dbPath, activeVar.name, [[entry.id, code]]).then(() => {
                      setClassifyEntries(prev => {
                        const next = [...prev];
                        next[classifyIndex] = { ...next[classifyIndex], value: code };
                        return next;
                      });
                      if (classifyIndex < classifyEntries.length - 1) setClassifyIndex(i => i + 1);
                    });
                  }}
                >
                  {getOptionLabel(activeVar, code)}
                </Button>
              ))}
            </div>
            
            <Html
              className="text-xs text-muted-foreground mt-4 block [&_kbd]:px-1.5 [&_kbd]:py-0.5 [&_kbd]:rounded [&_kbd]:border [&_kbd]:bg-muted [&_kbd]:font-mono"
              html={t('variables.classifier.hint')}
            />
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold tracking-tight">{t('variables.title')}</h1>

      <Tabs
        defaultValue="build"
        onValueChange={(value) => {
          if (value === 'cards' && cardImages.length === 0) loadCards();
        }}
      >
        <TabsList>
          <TabsTrigger value="build">{t('variables.tabs.build')}</TabsTrigger>
          <TabsTrigger value="classify">{t('variables.tabs.classify')}</TabsTrigger>
          <TabsTrigger value="cards">{t('variables.tabs.cards')}</TabsTrigger>
        </TabsList>

      <TabsContent value="build" className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('variables.create.title')}</CardTitle>
        </CardHeader>
        <CardContent>
          <VariableBuilder busy={createBusy} onCreate={handleCreate} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('variables.list.title')}</CardTitle>
        </CardHeader>
        <CardContent>
          {variables.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('variables.list.empty')}</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted">
                  <tr>
                    <th className="px-4 py-2 font-medium">{t('variables.field.name')}</th>
                    <th className="px-4 py-2 font-medium">{t('variables.field.type')}</th>
                    <th className="px-4 py-2 font-medium">{t('variables.list.colRange')}</th>
                    <th className="px-4 py-2 font-medium"></th>
                  </tr>
                </thead>
                <tbody className="divide-y">
                  {variables.map(v => (
                    <tr key={v.name} className="hover:bg-muted/50">
                      <td className="px-4 py-2">{v.name}</td>
                      <td className="px-4 py-2">{v.var_type}</td>
                      <td className="px-4 py-2">
                        {v.var_type === 'ordinal' 
                          ? `${v.min_value ?? '?'} – ${v.max_value ?? '?'}`
                          : v.categories.map(c => `${c.label}=${c.code}`).join(', ')
                        }
                      </td>
                      <td className="px-4 py-2 text-right">
                        <Button variant="ghost" size="icon" onClick={() => handleDelete(v.name)} className="text-destructive">
                          <Trash2 className="w-4 h-4" />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="classify">
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('variables.classify.title')}</CardTitle>
          <CardDescription>{t('variables.classify.description')}</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-end gap-4">
            <div className="space-y-1 flex-1">
              <Label htmlFor="classify-variable-select">{t('variables.field.variable')}</Label>
              <Select value={classifyVar} onValueChange={setClassifyVar} disabled={variables.length === 0}>
                <SelectTrigger id="classify-variable-select"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {variables.map(v => <SelectItem key={v.name} value={v.name}>{v.name}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            <Button onClick={startClassify} disabled={!classifyVar}>
              {t('variables.classify.start')}
            </Button>
          </div>
        </CardContent>
      </Card>
      </TabsContent>

      <TabsContent value="cards">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <CardTitle className="text-lg">{t('variables.cards.title')}</CardTitle>
            <CardDescription>{t('variables.cards.description')}</CardDescription>
          </div>
          <Button variant="ghost" size="sm" onClick={loadCards} disabled={cardsLoading}>
            {t('common.refresh')}
          </Button>
        </CardHeader>
        <CardContent>
          {variables.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('variables.cards.noVariables')}</p>
          ) : cardsLoading && cardImages.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
          ) : cardImages.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t('variables.cards.empty')}</p>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {cardImages.map(img => (
                <div key={img.id} className="border rounded-md overflow-hidden bg-card shadow-sm">
                  <div className="relative aspect-[4/3] bg-muted">
                    {cardThumbnails[img.id] ? (
                      <img
                        src={cardThumbnails[img.id]}
                        className="w-full h-full object-cover cursor-zoom-in"
                        onClick={() => openLightbox(cardThumbnails[img.id], img.file_path)}
                      />
                    ) : (
                      <Skeleton className="w-full h-full rounded-none" />
                    )}
                    <button
                      type="button"
                      title={t('photoDetails.viewDetails')}
                      onClick={(e) => {
                        e.stopPropagation();
                        setCardsDetailsImageId(img.id);
                      }}
                      className="absolute top-1 right-1 w-6 h-6 flex items-center justify-center rounded-full bg-background/80 text-foreground hover:bg-background"
                    >
                      <Info className="w-3.5 h-3.5" />
                    </button>
                  </div>
                  <div className="p-3 space-y-2">
                    <div className="truncate text-sm font-medium" title={img.file_path}>
                      {img.file_path.split(/[\\/]/).pop()}
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {variables.map(v => {
                        const value = img.values[v.name];
                        return (
                          <span
                            key={v.name}
                            className={
                              value == null
                                ? 'text-xs px-2 py-0.5 rounded-full bg-muted text-muted-foreground'
                                : 'text-xs px-2 py-0.5 rounded-full bg-primary/10 text-primary'
                            }
                            title={v.name}
                          >
                            {v.name}: {value == null ? '—' : getOptionLabel(v, value)}
                          </span>
                        );
                      })}
                    </div>
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
        imageId={cardsDetailsImageId}
        open={cardsDetailsImageId !== null}
        onOpenChange={(open) => !open && setCardsDetailsImageId(null)}
      />
    </div>
  );
}
