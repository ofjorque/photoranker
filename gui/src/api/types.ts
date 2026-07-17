// Formas exactas de `data` por comando, verificadas contra core-cli/src
// (no solo contra la documentación) — ver docs/cli-reference.md.

export interface InitResult {
  scanned: number;
  inserted_ok: number;
  inserted_failed: number;
  skipped_existing: number;
  paired_raw_jpeg: number;
}

export interface PruneResult {
  db_path: string;
  marked_missing: number;
}

export interface BurstDetectResult {
  candidates_considered: number;
  bursts_created: number;
  images_grouped: number;
}

export interface BurstTournamentResult {
  burst_id: number;
  representative_image_id: number;
  rejected: number;
}

export interface ClusterPreviewResult {
  bic_by_k: Record<string, number>;
  [key: string]: unknown;
}

export interface ClusterCommitResult {
  clusters?: number;
  excluded_zero_variance?: string[];
  excluded_variables?: string[];
  /** Informativo, no excluye: la variable se duplicó internamente para
   *  evitar un crash de clustMD con bloques categóricos de ancho 1, pero
   *  sigue participando del clustering (ver docs/fase2-clustering.md). */
  duplicated_solo_categorical?: string;
  /** true si se reutilizó un modelo ya ajustado en un --preview anterior en
   *  vez de volver a correr clustMD (ver docs/fase2-clustering.md, "Caché de
   *  modelos ajustados"). */
  from_cache?: boolean;
  /** Cuántas imágenes quedaron con cluster_id=NULL por no superar
   *  --probability-threshold (0 si el umbral está deshabilitado). */
  below_probability_threshold?: number;
  [key: string]: unknown;
}

export interface ClusterRenameResult {
  id: number;
  name: string;
}

export interface TournamentImage {
  id: number;
  file_path: string;
  mu: number;
  sigma: number;
}

export interface TournamentNextResult {
  group_id: string;
  images: TournamentImage[];
}

export interface TournamentUpdatedImage {
  id: number;
  rank_position: number;
  mu: number;
  sigma: number;
}

export interface TournamentResultResult {
  group_id: string;
  updated: TournamentUpdatedImage[];
  global_sync: { flushed: number; pending: number };
}

export interface RankingEntry {
  id: number;
  file_path: string;
  mu: number;
  sigma: number;
  rejected: boolean;
  stalled: boolean;
}

export type TournamentStopStatus = 'converged' | 'timeout' | 'stalled' | 'in_progress';

export interface TournamentStatusResult {
  total_images: number;
  active_images: number;
  stalled_images: number;
  converged_images: number;
  convergence_ratio: number;
  rounds_completed: number;
  max_rounds: number;
  status: TournamentStopStatus;
}

export interface ExportXmpResult {
  written: number;
  excluded_failed_thumbnail: number;
  excluded_missing: number;
  mode: 'quantile' | 'fixed_provisional';
  fallback_fixed_mapping_used: number;
  stars_breakdown: Record<string, number>;
}

export interface FailedThumbnail {
  id: number;
  file_path: string;
}

export interface RetryThumbnailResult {
  id: number;
  thumbnail_status: 'ok';
}

export interface ResyncGlobalResult {
  project_id: string;
  source_db_path: string;
  rows_updated: number;
}

export interface VariableCategory {
  code: number;
  label: string;
}

export interface VariableCreateResult {
  id: number;
  name: string;
  var_type: 'ordinal' | 'nominal';
  position: number;
}

export interface VariableDeleteResult {
  variable: string;
  values_deleted: number;
}

export interface UserVariable {
  id: number;
  name: string;
  var_type: 'ordinal' | 'nominal';
  position: number;
  min_value: number | null;
  max_value: number | null;
  categories: VariableCategory[];
}

export interface VariableSetResult {
  variable: string;
  values_set: number;
}

export interface QualityMetrics {
  sharpness: number;
  brightness: number;
  contrast: number;
  overexposed_pct: number;
  underexposed_pct: number;
  saturation: number;
  colorfulness: number;
  entropy: number;
  average_r: number;
  average_g: number;
  average_b: number;
  orientation: 'portrait' | 'landscape' | 'square';
}

export interface GetQualityMetricsResult {
  id: number;
  metrics: QualityMetrics | null;
}

export interface GetThumbnailResult {
  id: number;
  thumbnail_b64: string;
}

export interface BurstImage {
  id: number;
  file_path: string;
}

export interface PendingBurst {
  id: number;
  images: BurstImage[];
}

export interface ResolvedBurstImage {
  id: number;
  file_path: string;
  rejected: boolean;
}

export interface ResolvedBurst {
  id: number;
  representative_image_id: number | null;
  images: ResolvedBurstImage[];
}

export interface BurstExcludeResult {
  burst_id: number;
  excluded: number[];
  burst_dissolved: boolean;
}

export interface BurstUndoResult {
  burst_id: number;
  reverted_images: number[];
  burst_status: 'pending' | 'completed';
}

export interface TournamentUndoResult {
  group_id: string;
  reverted_images: number[];
}

export interface TournamentResetResult {
  images_reset: number;
}

export interface ResetGlobalIndexResult {
  rows_deleted: number;
}

export interface ClusterRepresentativeImage {
  id: number;
  file_path: string;
  probability: number | null;
}

export interface ClusterSummary {
  id: number;
  name: string | null;
  member_count: number;
  representative_images: ClusterRepresentativeImage[];
}

export interface VariableValueEntry {
  id: number;
  file_path: string;
  value: number | null;
}

export interface DuplicateMatch {
  local_image_id: number;
  local_file_path: string;
  other_project_id: string;
  other_file_path: string;
  other_source_db_path: string | null;
  distance: number;
  exact: boolean;
}
