// Pantalla de inicio: elegir/abrir carpeta, correr init/prune/burst-detect,
// y los controles de deshacer/reiniciar agregados por feedback de uso real
// (ver docs/fase3-torneo.md, "Deshacer / reiniciar"). Envuelve exactamente
// los comandos del CLI, sin lógica nueva.
import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { cli, CliError, extractLogStatus } from '@/api';
import { getProject, setProject, dbPathFor } from '@/state';
import { showToast } from '@/toast';
import { navigate } from '@/router';
import { t } from '@/i18n';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from '@/components/ui/card';
import { FolderOpen, Undo2, Database, RefreshCw, Layers, Trophy } from 'lucide-react';
import { confirmDialog } from '@/components/ConfirmDialog';
import { showLoadingOverlay } from '@/components/LoadingOverlay';
import { Html } from '@/components/Html';

function initPhases(): string[] {
  return [
    t('home.init.phase.scanning'),
    t('home.init.phase.thumbnails'),
    t('home.init.phase.phash'),
    t('home.init.phase.quality'),
    t('home.init.phase.pairing'),
  ];
}

export function HomeView() {
  const [project, setProjectState] = useState(() => getProject());
  const [folderInput, setFolderInput] = useState(project?.folderPath ?? '');
  const [isBusy, setIsBusy] = useState(false);
  const [result, setResult] = useState<{ title: string; data: unknown } | null>(null);

  const handlePickFolder = async () => {
    const picked = await invoke<string | null>('pick_folder');
    if (picked) setFolderInput(picked);
  };

  const handleInit = async () => {
    const folder = folderInput.trim();
    if (!folder) {
      showToast(t('home.folder.needFolder'), true);
      return;
    }

    const run = cli.initAsync(folder, (_stream, line) => {
      const status = extractLogStatus(line);
      if (status) overlayHandle.setStatus(status);
    });
    const overlayHandle = showLoadingOverlay(t('home.init.loadingTitle'), initPhases(), {
      onCancel: () => run.cancel(),
    });

    setIsBusy(true);
    try {
      const data = await run.promise;
      const nextProject = { folderPath: folder, dbPath: dbPathFor(folder) };
      setProject(nextProject);
      setProjectState(nextProject);
      const pairedNote = data.paired_raw_jpeg > 0 ? t('home.init.pairedNote', { count: data.paired_raw_jpeg }) : '';
      showToast(
        t('home.init.result', {
          ok: data.inserted_ok,
          existing: data.skipped_existing,
          failed: data.inserted_failed,
          pairedNote,
        }),
      );
    } catch (e) {
      if (e instanceof CliError && e.code === 'CANCELLED') {
        showToast(t('home.init.cancelled'));
      } else {
        showToast(e instanceof CliError ? e.message : String(e), true);
      }
    } finally {
      overlayHandle.close();
      setIsBusy(false);
    }
  };

  const runCliAction = async (
    title: string,
    action: () => Promise<any>,
    confirmProps?: Parameters<typeof confirmDialog>[0],
  ) => {
    if (confirmProps) {
      const confirmed = await confirmDialog(confirmProps);
      if (!confirmed) return;
    }
    setIsBusy(true);
    try {
      const data = await action();
      setResult({ title, data });
      return data;
    } catch (e) {
      showToast(e instanceof CliError ? e.message : String(e), true);
    } finally {
      setIsBusy(false);
    }
  };

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6">
      <h1 className="text-3xl font-bold tracking-tight">{t('home.title')}</h1>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">{t('home.folder.label')}</CardTitle>
        </CardHeader>
        <CardContent className="flex gap-4">
          <Input
            value={folderInput}
            onChange={(e) => setFolderInput(e.target.value)}
            placeholder={t('home.folder.placeholder')}
            className="flex-1"
          />
          <Button variant="secondary" onClick={handlePickFolder} disabled={isBusy}>
            <FolderOpen className="w-4 h-4 mr-2" />
            {t('home.folder.pick')}
          </Button>
        </CardContent>
        <CardFooter>
          <Button onClick={handleInit} disabled={isBusy} className="w-full sm:w-auto">
            {t('home.init.button')}
          </Button>
        </CardFooter>
      </Card>

      {project && (
        <>
          <Card>
            <CardHeader>
              <CardTitle className="text-lg">
                <Html html={t('home.actions.heading', { folderPath: project.folderPath })} />
              </CardTitle>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-4">
              <Button
                variant="outline"
                disabled={isBusy}
                onClick={async () => {
                  const data = await runCliAction('prune', () => cli.prune(project.dbPath));
                  if (data) showToast(t('home.prune.result', { count: data.marked_missing }));
                }}
              >
                prune
              </Button>
              <Button
                variant="outline"
                disabled={isBusy}
                onClick={async () => {
                  const data = await runCliAction('burst-detect', () => cli.burstDetect(project.dbPath));
                  if (data) showToast(t('home.burstDetect.result', { count: data.bursts_created }));
                }}
              >
                burst-detect
              </Button>
              <Button onClick={() => navigate('bursts')} disabled={isBusy}>
                <Layers className="w-4 h-4 mr-2" />
                {t('home.actions.gotoBursts')}
              </Button>
              <Button onClick={() => navigate('tournament')} disabled={isBusy}>
                <Trophy className="w-4 h-4 mr-2" />
                {t('home.actions.gotoTournament')}
              </Button>
            </CardContent>
          </Card>

          {result && (
            <Card className="bg-muted">
              <CardHeader>
                <CardTitle className="text-md">{result.title}</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="text-xs overflow-auto">{JSON.stringify(result.data, null, 2)}</pre>
              </CardContent>
            </Card>
          )}

          <Card className="border-destructive/50">
            <CardHeader>
              <CardTitle className="text-lg text-destructive">{t('home.dangerZone.title')}</CardTitle>
              <p className="text-sm text-muted-foreground">{t('home.dangerZone.description')}</p>
            </CardHeader>
            <CardContent className="flex flex-wrap gap-4">
              <Button
                variant="outline"
                disabled={isBusy}
                onClick={async () => {
                  const data = await runCliAction('tournament-undo', () => cli.tournamentUndo(project.dbPath));
                  if (data) {
                    showToast(
                      t('home.undo.result', { groupId: data.group_id.slice(0, 8), count: data.reverted_images.length }),
                    );
                  }
                }}
              >
                <Undo2 className="w-4 h-4 mr-2" />
                {t('home.dangerZone.undo')}
              </Button>
              <Button
                variant="destructive"
                disabled={isBusy}
                onClick={async () => {
                  const data = await runCliAction('tournament-reset', () => cli.tournamentReset(project.dbPath), {
                    title: t('home.resetTournament.confirmTitle'),
                    message: t('home.resetTournament.confirmMessage', { folderPath: project.folderPath }),
                    confirmLabel: t('home.resetTournament.confirmLabel'),
                    danger: true,
                  });
                  if (data) showToast(t('home.resetTournament.result', { count: data.images_reset }));
                }}
              >
                {t('home.dangerZone.reset')}
              </Button>
            </CardContent>
          </Card>
        </>
      )}

      <Card className="border-destructive/50">
        <CardHeader>
          <CardTitle className="text-lg text-destructive">{t('home.globalIndex.title')}</CardTitle>
          <Html className="text-sm text-muted-foreground" html={t('home.globalIndex.description')} />
        </CardHeader>
        <CardContent className="flex flex-wrap gap-4">
          {project && (
            <Button
              variant="outline"
              disabled={isBusy}
              title={t('home.globalIndex.resyncTitle')}
              onClick={async () => {
                const data = await runCliAction('resync-global', () => cli.resyncGlobal(project.folderPath));
                if (data) showToast(t('home.resyncGlobal.result', { count: data.rows_updated }));
              }}
            >
              <RefreshCw className="w-4 h-4 mr-2" />
              resync-global
            </Button>
          )}
          <Button
            variant="destructive"
            disabled={isBusy}
            title={t('home.globalIndex.resetTitle')}
            onClick={async () => {
              const data = await runCliAction('reset-global-index', () => cli.resetGlobalIndex(), {
                title: t('home.resetGlobal.confirmTitle'),
                message: t('home.resetGlobal.confirmMessage'),
                confirmLabel: t('home.resetGlobal.confirmLabel'),
                danger: true,
              });
              if (data) showToast(t('home.resetGlobal.result', { count: data.rows_deleted }));
            }}
          >
            <Database className="w-4 h-4 mr-2" />
            reset-global-index
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
