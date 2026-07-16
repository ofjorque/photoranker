// Wrappers de alto nivel, un método por subcomando del CLI — ver
// docs/cli-reference.md. Cada uno solo arma los args de `photoranker` y tipa
// la respuesta; ninguna decisión de negocio vive acá (ver CLAUDE.md).
import { callCli, formatRankingArgs } from './cli';
import type {
  BurstDetectResult,
  BurstTournamentResult,
  ClusterCommitResult,
  ClusterPreviewResult,
  ClusterRenameResult,
  ClusterSummary,
  ExportXmpResult,
  FailedThumbnail,
  GetQualityMetricsResult,
  GetThumbnailResult,
  InitResult,
  PendingBurst,
  PruneResult,
  RankingEntry,
  ResetGlobalIndexResult,
  ResyncGlobalResult,
  RetryThumbnailResult,
  TournamentNextResult,
  TournamentResetResult,
  TournamentResultResult,
  TournamentStatusResult,
  TournamentUndoResult,
  UserVariable,
  VariableCreateResult,
  VariableSetResult,
  VariableValueEntry,
} from './types';

function dbArgs(dbPath: string): string[] {
  return ['--db', dbPath];
}

export const cli = {
  init: (path: string) => callCli<InitResult>(['init', '--path', path]),

  prune: (dbPath: string) => callCli<PruneResult>(['prune', ...dbArgs(dbPath)]),

  burstDetect: (dbPath: string, threshold?: number) =>
    callCli<BurstDetectResult>([
      'burst-detect',
      ...(threshold != null ? ['--threshold', String(threshold)] : []),
      ...dbArgs(dbPath),
    ]),

  listBursts: (dbPath: string) => callCli<PendingBurst[]>(['list-bursts', ...dbArgs(dbPath)]),

  burstTournament: (dbPath: string, burstId: number, ranking: Array<[number, number]>) =>
    callCli<BurstTournamentResult>([
      'burst-tournament',
      '--burst-id',
      String(burstId),
      ...dbArgs(dbPath),
      '--ranking',
      ...formatRankingArgs(ranking),
    ]),

  clusterPreview: (dbPath: string) =>
    callCli<ClusterPreviewResult>(['cluster', '--preview', ...dbArgs(dbPath)]),

  clusterCommit: (dbPath: string, k?: number) =>
    callCli<ClusterCommitResult>([
      'cluster',
      ...(k != null ? ['--k', String(k)] : []),
      ...dbArgs(dbPath),
    ]),

  clusterRename: (dbPath: string, id: number, name: string) =>
    callCli<ClusterRenameResult>([
      'cluster-rename',
      '--id',
      String(id),
      '--name',
      name,
      ...dbArgs(dbPath),
    ]),

  tournamentNext: (dbPath: string) =>
    callCli<TournamentNextResult | null>(['tournament-next', ...dbArgs(dbPath)]),

  tournamentResult: (dbPath: string, groupId: string, ranking: Array<[number, number]>) =>
    callCli<TournamentResultResult>([
      'tournament-result',
      '--group-id',
      groupId,
      ...dbArgs(dbPath),
      '--ranking',
      ...formatRankingArgs(ranking),
    ]),

  ranking: (dbPath: string) => callCli<RankingEntry[]>(['ranking', ...dbArgs(dbPath)]),

  tournamentStatus: (dbPath: string) =>
    callCli<TournamentStatusResult>(['tournament-status', ...dbArgs(dbPath)]),

  tournamentUndo: (dbPath: string) =>
    callCli<TournamentUndoResult>(['tournament-undo', ...dbArgs(dbPath)]),

  tournamentReset: (dbPath: string) =>
    callCli<TournamentResetResult>(['tournament-reset', ...dbArgs(dbPath)]),

  resetGlobalIndex: () => callCli<ResetGlobalIndexResult>(['reset-global-index']),

  listClusters: (dbPath: string) => callCli<ClusterSummary[]>(['list-clusters', ...dbArgs(dbPath)]),

  listFailedThumbnails: (dbPath: string) =>
    callCli<FailedThumbnail[]>(['list-failed-thumbnails', ...dbArgs(dbPath)]),

  retryThumbnail: (dbPath: string, imageId: number) =>
    callCli<RetryThumbnailResult>([
      'retry-thumbnail',
      '--image-id',
      String(imageId),
      ...dbArgs(dbPath),
    ]),

  exportXmp: (dbPath: string) => callCli<ExportXmpResult>(['export-xmp', ...dbArgs(dbPath)]),

  resyncGlobal: (path: string) => callCli<ResyncGlobalResult>(['resync-global', '--path', path]),

  variableCreate: (
    dbPath: string,
    name: string,
    varType: 'ordinal' | 'nominal',
    opts: { min?: number; max?: number; categories?: string },
  ) =>
    callCli<VariableCreateResult>([
      'variable-create',
      '--name',
      name,
      '--type',
      varType,
      ...(opts.min != null ? ['--min', String(opts.min)] : []),
      ...(opts.max != null ? ['--max', String(opts.max)] : []),
      ...(opts.categories ? ['--categories', opts.categories] : []),
      ...dbArgs(dbPath),
    ]),

  variableList: (dbPath: string) => callCli<UserVariable[]>(['variable-list', ...dbArgs(dbPath)]),

  variableSet: (dbPath: string, variable: string, values: Array<[number, number]>) =>
    callCli<VariableSetResult>([
      'variable-set',
      '--variable',
      variable,
      ...dbArgs(dbPath),
      '--values',
      ...values.map(([id, v]) => `${id}:${v}`),
    ]),

  getVariableValues: (dbPath: string, variable: string) =>
    callCli<VariableValueEntry[]>([
      'get-variable-values',
      '--variable',
      variable,
      ...dbArgs(dbPath),
    ]),

  getThumbnail: (dbPath: string, imageId: number) =>
    callCli<GetThumbnailResult>(['get-thumbnail', '--image-id', String(imageId), ...dbArgs(dbPath)]),

  getQualityMetrics: (dbPath: string, imageId: number) =>
    callCli<GetQualityMetricsResult>([
      'get-quality-metrics',
      '--image-id',
      String(imageId),
      ...dbArgs(dbPath),
    ]),
};

export { CliError } from './cli';
export * from './types';
