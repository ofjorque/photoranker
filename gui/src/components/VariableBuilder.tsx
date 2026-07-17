// Constructor de variables custom por drag-and-drop — reemplaza el
// formulario plano (nombre + Select ordinal/nominal + min/max/categorías) de
// `views/Variables.tsx` por una interacción de "arrastrar un tipo de bloque
// a una zona, después configurarlo". El contrato con el CLI no cambia: al
// confirmar, produce un array de {name, varType, min?, max?, categories?},
// una entrada por cada variable_create que espera `cli.variableCreate` — este
// componente es puramente de presentación, la llamada al CLI (una por
// variable, en secuencia) sigue en Variables.tsx. Permite encolar varias
// definiciones antes de confirmar (ver docs/fase7-mejoras-post-mvp.md, "Crear
// varias variables custom de una sola pasada").
import { useState } from 'react';
import { DndContext, useDraggable, useDroppable, type DragEndEvent } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { ListOrdered, Tags, ArrowLeft, Trash2, Plus, GripVertical } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { t } from '@/i18n';
import { showToast } from '@/toast';
import { cn } from '@/lib/utils';

interface CategoryRow {
  id: string;
  label: string;
}

export type VariableType = 'ordinal' | 'nominal';

export interface VariableBuilderInput {
  name: string;
  varType: VariableType;
  min?: number;
  max?: number;
  categories?: string;
}

export interface VariableBuilderProps {
  busy: boolean;
  onCreate: (inputs: VariableBuilderInput[]) => void;
}

const BLOCKS: Array<{ type: VariableType; icon: typeof ListOrdered; titleKey: string; descKey: string }> = [
  { type: 'ordinal', icon: ListOrdered, titleKey: 'variableBuilder.block.ordinal.title', descKey: 'variableBuilder.block.ordinal.description' },
  { type: 'nominal', icon: Tags, titleKey: 'variableBuilder.block.nominal.title', descKey: 'variableBuilder.block.nominal.description' },
];

function DraggableBlock({ type, icon: Icon, titleKey, descKey }: (typeof BLOCKS)[number]) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({ id: type });
  return (
    <div
      ref={setNodeRef}
      {...listeners}
      {...attributes}
      style={{ transform: CSS.Translate.toString(transform) }}
      className={cn(
        'flex items-center gap-3 rounded-lg border bg-card p-4 cursor-grab active:cursor-grabbing select-none touch-none',
        isDragging && 'opacity-50 z-10',
      )}
    >
      <Icon className="w-6 h-6 text-primary shrink-0" />
      <div>
        <div className="font-medium text-sm">{t(titleKey)}</div>
        <div className="text-xs text-muted-foreground">{t(descKey)}</div>
      </div>
    </div>
  );
}

function DropZone({ children, isEmpty }: { children: React.ReactNode; isEmpty: boolean }) {
  const { setNodeRef, isOver } = useDroppable({ id: 'variable-builder-dropzone' });
  return (
    <div
      ref={setNodeRef}
      className={cn(
        'rounded-lg border-2 border-dashed p-6 transition-colors',
        isEmpty && 'flex items-center justify-center text-muted-foreground text-sm min-h-[120px]',
        isOver ? 'border-primary bg-primary/5' : 'border-border',
      )}
    >
      {children}
    </div>
  );
}

function CategoryChipRow({
  row,
  code,
  onLabelChange,
  onRemove,
}: {
  row: CategoryRow;
  code: number;
  onLabelChange: (id: string, label: string) => void;
  onRemove: (id: string) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: row.id });
  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition }}
      className={cn(
        'flex items-center gap-2 rounded-md border bg-card px-2 py-1.5',
        isDragging && 'opacity-50 z-10',
      )}
    >
      <button
        type="button"
        {...listeners}
        {...attributes}
        className="cursor-grab active:cursor-grabbing text-muted-foreground touch-none shrink-0"
        title={t('variableBuilder.categories.dragHandle')}
      >
        <GripVertical className="w-4 h-4" />
      </button>
      <Input
        value={row.label}
        onChange={(e) => onLabelChange(row.id, e.target.value)}
        placeholder={t('variableBuilder.categories.labelPlaceholder')}
        className="h-8 flex-1"
      />
      <span className="text-xs text-muted-foreground shrink-0 whitespace-nowrap">
        {t('variableBuilder.categories.code', { code })}
      </span>
      <button
        type="button"
        onClick={() => onRemove(row.id)}
        className="text-muted-foreground hover:text-destructive shrink-0"
        title={t('common.remove')}
      >
        <Trash2 className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}

export function VariableBuilder({ busy, onCreate }: VariableBuilderProps) {
  const [selectedType, setSelectedType] = useState<VariableType | null>(null);
  const [name, setName] = useState('');
  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [categoryRows, setCategoryRows] = useState<CategoryRow[]>([]);
  const [newCategoryLabel, setNewCategoryLabel] = useState('');
  const [queue, setQueue] = useState<VariableBuilderInput[]>([]);

  const handleDragEnd = (event: DragEndEvent) => {
    if (event.over?.id === 'variable-builder-dropzone') {
      setSelectedType(event.active.id as VariableType);
      return;
    }
    if (event.over && event.active.id !== event.over.id) {
      setCategoryRows((prev) => {
        const oldIndex = prev.findIndex((r) => r.id === event.active.id);
        const newIndex = prev.findIndex((r) => r.id === event.over!.id);
        if (oldIndex === -1 || newIndex === -1) return prev;
        return arrayMove(prev, oldIndex, newIndex);
      });
    }
  };

  const reset = () => {
    setSelectedType(null);
    setName('');
    setMin('');
    setMax('');
    setCategoryRows([]);
    setNewCategoryLabel('');
  };

  const addCategory = () => {
    const trimmed = newCategoryLabel.trim();
    if (!trimmed) return;
    setCategoryRows((prev) => [...prev, { id: crypto.randomUUID(), label: trimmed }]);
    setNewCategoryLabel('');
  };

  const handleAddToQueue = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      showToast(t('variables.create.nameRequired'), true);
      return;
    }
    if (queue.some((q) => q.name === trimmed)) {
      showToast(t('variableBuilder.queue.duplicateName', { name: trimmed }), true);
      return;
    }
    if (!selectedType) return;
    if (selectedType === 'nominal' && categoryRows.some((r) => !r.label.trim())) {
      showToast(t('variableBuilder.categories.emptyLabel'), true);
      return;
    }
    setQueue((prev) => [
      ...prev,
      {
        name: trimmed,
        varType: selectedType,
        min: selectedType === 'ordinal' && min !== '' ? Number(min) : undefined,
        max: selectedType === 'ordinal' && max !== '' ? Number(max) : undefined,
        categories:
          selectedType === 'nominal' && categoryRows.length > 0
            ? categoryRows.map((r, i) => `${r.label.trim()}:${i}`).join(',')
            : undefined,
      },
    ]);
    reset();
  };

  const removeFromQueue = (name: string) => {
    setQueue((prev) => prev.filter((q) => q.name !== name));
  };

  const handleSubmitQueue = () => {
    if (queue.length === 0) return;
    onCreate(queue);
    setQueue([]);
  };

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">{t('variableBuilder.intro')}</p>

      <DndContext onDragEnd={handleDragEnd}>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {BLOCKS.map((block) => (
            <DraggableBlock key={block.type} {...block} />
          ))}
        </div>

        <DropZone isEmpty={selectedType === null}>
          {selectedType === null ? (
            t('variableBuilder.dropzone.empty')
          ) : (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">
                  {t('variableBuilder.dropzone.configuring', {
                    type: t(selectedType === 'ordinal' ? 'variableBuilder.block.ordinal.title' : 'variableBuilder.block.nominal.title'),
                  })}
                </span>
                <Button variant="ghost" size="sm" onClick={reset}>
                  <ArrowLeft className="w-4 h-4 mr-1" />
                  {t('variableBuilder.changeType')}
                </Button>
              </div>

              <div className="space-y-1">
                <Label htmlFor="variable-name-input">{t('variables.field.name')}</Label>
                <Input
                  id="variable-name-input"
                  placeholder={t('variables.create.namePlaceholder')}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </div>

              {selectedType === 'ordinal' ? (
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-1">
                    <Label htmlFor="variable-min-input">{t('variables.field.min')}</Label>
                    <Input id="variable-min-input" type="number" value={min} onChange={(e) => setMin(e.target.value)} />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="variable-max-input">{t('variables.field.max')}</Label>
                    <Input id="variable-max-input" type="number" value={max} onChange={(e) => setMax(e.target.value)} />
                  </div>
                </div>
              ) : (
                <div className="space-y-1">
                  <Label>{t('variables.create.categoriesLabel')}</Label>
                  <div className="space-y-2 rounded-lg border p-3">
                    {categoryRows.length > 0 && (
                      <SortableContext items={categoryRows.map((r) => r.id)} strategy={verticalListSortingStrategy}>
                        <div className="space-y-1.5">
                          {categoryRows.map((row, i) => (
                            <CategoryChipRow
                              key={row.id}
                              row={row}
                              code={i}
                              onLabelChange={(id, label) =>
                                setCategoryRows((prev) => prev.map((r) => (r.id === id ? { ...r, label } : r)))
                              }
                              onRemove={(id) => setCategoryRows((prev) => prev.filter((r) => r.id !== id))}
                            />
                          ))}
                        </div>
                      </SortableContext>
                    )}
                    <div className="flex gap-2">
                      <Input
                        value={newCategoryLabel}
                        onChange={(e) => setNewCategoryLabel(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            addCategory();
                          }
                        }}
                        placeholder={t('variableBuilder.categories.newPlaceholder')}
                        className="h-8"
                      />
                      <Button type="button" variant="outline" size="sm" onClick={addCategory}>
                        <Plus className="w-3.5 h-3.5 mr-1" />
                        {t('variableBuilder.categories.add')}
                      </Button>
                    </div>
                  </div>
                </div>
              )}

              <Button onClick={handleAddToQueue} disabled={busy} variant="secondary">
                <Plus className="w-4 h-4 mr-1" />
                {t('variableBuilder.addToQueue')}
              </Button>
            </div>
          )}
        </DropZone>
      </DndContext>

      {queue.length > 0 && (
        <div className="space-y-3 rounded-lg border p-4">
          <div className="text-sm font-medium">{t('variableBuilder.queue.title', { count: queue.length })}</div>
          <ul className="space-y-1.5">
            {queue.map((q) => (
              <li key={q.name} className="flex items-center justify-between gap-2 text-sm bg-muted/50 rounded px-3 py-1.5">
                <span className="truncate">
                  <span className="font-medium">{q.name}</span>{' '}
                  <span className="text-muted-foreground">
                    ({q.varType === 'ordinal' ? t('variableBuilder.block.ordinal.title') : t('variableBuilder.block.nominal.title')})
                  </span>
                </span>
                <button
                  type="button"
                  onClick={() => removeFromQueue(q.name)}
                  className="text-muted-foreground hover:text-destructive shrink-0"
                  title={t('common.remove')}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </li>
            ))}
          </ul>
          <Button onClick={handleSubmitQueue} disabled={busy}>
            {t('variableBuilder.submit', { count: queue.length })}
          </Button>
        </div>
      )}
    </div>
  );
}
