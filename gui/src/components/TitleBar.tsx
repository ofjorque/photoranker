import { getCurrentWindow } from '@tauri-apps/api/window';
import { Minus, Square, Copy, X } from 'lucide-react';
import { useEffect, useState } from 'react';

export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);
  
  useEffect(() => {
    const checkMaximized = async () => {
      const maximized = await getCurrentWindow().isMaximized();
      setIsMaximized(maximized);
    };
    
    checkMaximized();
    
    // Listen to resize events to update the maximized state
    const unlisten = getCurrentWindow().onResized(async () => {
      checkMaximized();
    });
    
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const minimize = () => getCurrentWindow().minimize();
  const toggleMaximize = async () => {
    const win = getCurrentWindow();
    if (await win.isMaximized()) {
      await win.unmaximize();
    } else {
      await win.maximize();
    }
  };
  const close = () => getCurrentWindow().close();

  return (
    <div 
      data-tauri-drag-region 
      className="h-8 flex justify-between items-center bg-background border-b select-none"
    >
      <div className="flex items-center pl-3 gap-2 pointer-events-none text-sm font-medium">
        <span>Photo<span className="text-primary">Ranker</span></span>
      </div>
      <div className="flex h-full">
        <button 
          onClick={minimize}
          className="h-full px-4 hover:bg-muted inline-flex items-center justify-center transition-colors"
          tabIndex={-1}
        >
          <Minus className="w-4 h-4" />
        </button>
        <button
          onClick={toggleMaximize}
          className="h-full px-4 hover:bg-muted inline-flex items-center justify-center transition-colors"
          tabIndex={-1}
        >
          {isMaximized ? <Copy className="w-3.5 h-3.5 -scale-x-100" /> : <Square className="w-3.5 h-3.5" />}
        </button>
        <button 
          onClick={close}
          className="h-full px-4 hover:bg-destructive hover:text-destructive-foreground inline-flex items-center justify-center transition-colors"
          tabIndex={-1}
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
