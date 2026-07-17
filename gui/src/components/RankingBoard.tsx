import { useState, useEffect, useRef } from 'react';
import { getThumbnailDataUrl } from '@/api/thumbnailCache';
import { isTypingTarget } from '@/utils/dom';
import { t } from '@/i18n';
import { Html } from '@/components/Html';
import { Button } from '@/components/ui/button';
import { openLightbox } from '@/components/Lightbox';
import { cn } from '@/lib/utils';

export interface RankingBoardImage {
  id: number;
  file_path: string;
}

export interface RankingBoardProps {
  dbPath: string;
  images: RankingBoardImage[];
  onSubmit: (ranking: Array<[number, number]>) => void;
  captionFor?: (img: RankingBoardImage) => string;
  onFocusChange?: (img: RankingBoardImage) => void;
}

export function RankingBoard({ dbPath, images, onSubmit, captionFor, onFocusChange }: RankingBoardProps) {
  const [positions, setPositions] = useState<Map<number, number>>(new Map());
  const [focusedIndex, setFocusedIndex] = useState(0);
  const [errorMsg, setErrorMsg] = useState('');
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Load thumbnails
    images.forEach(img => {
      getThumbnailDataUrl(dbPath, img.id).then(url => {
        if (url) {
          setThumbnails(prev => ({ ...prev, [img.id]: url }));
        }
      });
    });
  }, [images, dbPath]);

  useEffect(() => {
    onFocusChange?.(images[focusedIndex]);
    
    // Scroll into view
    if (containerRef.current) {
      const cards = containerRef.current.querySelectorAll('.ranking-card');
      const focusedCard = cards[focusedIndex];
      if (focusedCard) {
        focusedCard.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }
  }, [focusedIndex, images, onFocusChange]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target as Element)) return;

      if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || (e.key === 'Tab' && !e.shiftKey)) {
        e.preventDefault();
        setFocusedIndex(i => (i + 1) % images.length);
      } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || (e.key === 'Tab' && e.shiftKey)) {
        e.preventDefault();
        setFocusedIndex(i => (i - 1 + images.length) % images.length);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        trySubmit();
      } else if (e.key === 'Backspace' || e.key.toLowerCase() === 'r') {
        e.preventDefault();
        setPositions(new Map());
        setErrorMsg('');
      } else {
        const n = Number(e.key);
        if (Number.isInteger(n) && n >= 1 && n <= images.length) {
          e.preventDefault();
          setPositions(prev => {
            const next = new Map(prev);
            next.set(images[focusedIndex].id, n);
            return next;
          });
          setErrorMsg('');
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  });

  const trySubmit = () => {
    const missing = images.filter((img) => !positions.has(img.id));
    if (missing.length > 0) {
      setErrorMsg(t('rankingBoard.missing', { count: missing.length }));
      return;
    }
    const ranking: Array<[number, number]> = images.map((img) => [img.id, positions.get(img.id)!]);
    onSubmit(ranking);
  };

  const getPositionCount = (pos: number) => {
    let count = 0;
    for (const v of positions.values()) {
      if (v === pos) count++;
    }
    return count;
  };

  return (
    <div className="flex flex-col h-full bg-background">
      <div 
        ref={containerRef}
        className="flex-1 overflow-auto p-4 grid gap-4 items-start"
        style={{ gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))' }}
      >
        {images.map((img, idx) => {
          const isFocused = idx === focusedIndex;
          const pos = positions.get(img.id);
          const isTied = pos !== undefined && getPositionCount(pos) > 1;
          const thumb = thumbnails[img.id];
          const name = img.file_path.split(/[\\/]/).pop() ?? img.file_path;

          return (
            <div 
              key={img.id} 
              onClick={() => setFocusedIndex(idx)}
              className={cn(
                "ranking-card relative flex flex-col rounded-md border-2 overflow-hidden transition-colors cursor-pointer bg-card text-card-foreground",
                isFocused ? "border-primary focus-halo" : "border-border hover:border-border/80"
              )}
            >
              <div className="relative aspect-[3/2] bg-muted flex items-center justify-center">
                {thumb ? (
                  <img
                    src={thumb}
                    alt={name}
                    className="w-full h-full object-contain cursor-zoom-in"
                    onClick={(e) => {
                      e.stopPropagation();
                      openLightbox(thumb, name);
                    }}
                  />
                ) : (
                  <span className="text-muted-foreground text-sm">{t('common.loading')}</span>
                )}
                {pos !== undefined && (
                  <div
                    title={isTied ? t('rankingBoard.badge.tied', { pos }) : t('rankingBoard.badge.position', { pos })}
                    className={cn(
                      "absolute top-2 right-2 w-8 h-8 rounded-full flex items-center justify-center font-bold shadow-md",
                      isTied
                        ? "bg-tie-badge text-tie-badge-foreground animate-tie-pulse"
                        : "bg-success text-success-foreground",
                    )}
                  >
                    {isTied ? `=${pos}` : pos}
                  </div>
                )}
              </div>
              <div className="p-2 text-xs truncate" title={img.file_path}>
                <div className="truncate font-medium">{name}</div>
                {captionFor && <div className="text-muted-foreground">{captionFor(img)}</div>}
              </div>
            </div>
          );
        })}
      </div>
      
      <div className="p-4 border-t bg-muted/30 flex items-center justify-between">
        <Html
          className="text-sm text-muted-foreground [&_kbd]:px-1.5 [&_kbd]:py-0.5 [&_kbd]:rounded [&_kbd]:border [&_kbd]:bg-muted [&_kbd]:font-mono [&_kbd]:text-xs"
          html={t('rankingBoard.hint.controls', { count: images.length })}
        />
        <div className="flex items-center gap-4">
          {errorMsg && <span className="text-destructive text-sm font-medium">{errorMsg}</span>}
          <Button onClick={trySubmit}>{t('rankingBoard.confirm')}</Button>
        </div>
      </div>
    </div>
  );
}
