// Constructor de variables custom por drag-and-drop — reemplaza el
// formulario plano (nombre + Select ordinal/nominal + min/max/categorías) de
// `views/Variables.tsx` por una interacción de "arrastrar un tipo de bloque
// a una zona, después configurarlo". El contrato con el CLI no cambia: al
// confirmar, produce exactamente {name, varType, min?, max?, categories?},
// lo mismo que ya esperaba `cli.variableCreate` — este componente es
// puramente de presentación, la llamada al CLI sigue en Variables.tsx.
import { useState } from 'react';
import { DndContext, useDraggable, useDroppable, type DragEndEvent } from '@dnd-kit/core';
import { CSS } from '@dnd-kit/utilities';
import { ListOrdered, Tags, ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { t } from '@/i18n';
import { showToast } from '@/toast';
import { cn } from '@/lib/utils';

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
  onCreate: (input: VariableBuilderInput) => void;
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

export function VariableBuilder({ busy, onCreate }: VariableBuilderProps) {
  const [selectedType, setSelectedType] = useState<VariableType | null>(null);
  const [name, setName] = useState('');
  const [min, setMin] = useState('');
  const [max, setMax] = useState('');
  const [categories, setCategories] = useState('');

  const handleDragEnd = (event: DragEndEvent) => {
    if (event.over?.id === 'variable-builder-dropzone') {
      setSelectedType(event.active.id as VariableType);
    }
  };

  const reset = () => {
    setSelectedType(null);
    setName('');
    setMin('');
    setMax('');
    setCategories('');
  };

  const handleSubmit = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      showToast(t('variables.create.nameRequired'), true);
      return;
    }
    if (!selectedType) return;
    onCreate({
      name: trimmed,
      varType: selectedType,
      min: selectedType === 'ordinal' && min !== '' ? Number(min) : undefined,
      max: selectedType === 'ordinal' && max !== '' ? Number(max) : undefined,
      categories: selectedType === 'nominal' && categories.trim() !== '' ? categories.trim() : undefined,
    });
    reset();
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
                  <Label htmlFor="variable-categories-input">{t('variables.create.categoriesLabel')}</Label>
                  <Input
                    id="variable-categories-input"
                    placeholder={t('variables.create.categoriesPlaceholder')}
                    value={categories}
                    onChange={(e) => setCategories(e.target.value)}
                  />
                </div>
              )}

              <Button onClick={handleSubmit} disabled={busy}>
                {t('variableBuilder.submit')}
              </Button>
            </div>
          )}
        </DropZone>
      </DndContext>
    </div>
  );
}
