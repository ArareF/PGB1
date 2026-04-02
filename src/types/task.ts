/** 任务系统共享类型 */

export interface GlobalTaskChild {
  name: string
}

export interface GlobalTask {
  name: string
  children: GlobalTaskChild[]
}

export interface GlobalTaskConfig {
  tasks: GlobalTask[]
}

export interface ApplyTaskResult {
  created: string[]
  archived: string[]
  errors: string[]
}

export interface ArchivedVersion {
  task_name: string
  timestamp: string
  display_time: string
  path: string
}
