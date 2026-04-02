/** 素材系统共享类型 */

export interface PreviewVideoEntry {
  name: string
  path: string
  extension: string
  size_bytes: number
  upload_status: 'uploaded' | 'outdated' | 'none'
}

export interface MaterialVersion {
  stage: string
  stage_label: string
  scale: string
  file_path: string
  folder_path: string
  extension: string
  size_bytes: number
}
