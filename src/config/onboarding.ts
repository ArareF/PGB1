/**
 * 首次启动引导 + 页面指引数据 SSOT
 * 所有文案通过 i18n key 引用，不硬编码
 */

/** 页面介绍条目（引导弹窗翻页用） */
export interface PageIntro {
  id: string
  titleKey: string
  descriptionKey: string
  screenshot: string | null  // 暂为 null，用户后续提供截图
}

/** 页面指引批注条目 */
export interface GuideAnnotation {
  id: string
  labelKey: string
  top: string       // CSS 值如 '15%'
  left: string      // CSS 值如 '50%'
  arrowDirection?: 'up' | 'down' | 'left' | 'right'
}

/** 引导弹窗中 9 个页面介绍 */
export const PAGE_INTROS: PageIntro[] = [
  { id: 'home',      titleKey: 'onboarding.pageHome',      descriptionKey: 'onboarding.pageHomeDesc',      screenshot: null },
  { id: 'project',   titleKey: 'onboarding.pageProject',   descriptionKey: 'onboarding.pageProjectDesc',   screenshot: null },
  { id: 'task',      titleKey: 'onboarding.pageTask',      descriptionKey: 'onboarding.pageTaskDesc',      screenshot: null },
  { id: 'taskList',  titleKey: 'onboarding.pageTaskList',  descriptionKey: 'onboarding.pageTaskListDesc',  screenshot: null },
  { id: 'gameIntro', titleKey: 'onboarding.pageGameIntro', descriptionKey: 'onboarding.pageGameIntroDesc', screenshot: null },
  { id: 'materials', titleKey: 'onboarding.pageMaterials', descriptionKey: 'onboarding.pageMaterialsDesc', screenshot: null },
  { id: 'scale',     titleKey: 'onboarding.pageScale',     descriptionKey: 'onboarding.pageScaleDesc',     screenshot: null },
  { id: 'convert',   titleKey: 'onboarding.pageConvert',   descriptionKey: 'onboarding.pageConvertDesc',   screenshot: null },
  { id: 'settings',  titleKey: 'onboarding.pageSettings',  descriptionKey: 'onboarding.pageSettingsDesc',  screenshot: null },
]

/** 各页面指引批注数据 */
export const PAGE_GUIDE_ANNOTATIONS: Record<string, GuideAnnotation[]> = {
  home: [
    { id: 'sidebar',     labelKey: 'pageGuide.homeSidebar',     top: '60%', left: '20%',  arrowDirection: 'left' },
    { id: 'project-list', labelKey: 'pageGuide.homeProjectList', top: '50%', left: '50%' },
    { id: 'add-project', labelKey: 'pageGuide.homeAddProject',  top: '27%', left: '31%', arrowDirection: 'left' },
    { id: 'more-menu',   labelKey: 'pageGuide.homeMoreMenu',    top: '13.5%',  left: '80%', arrowDirection: 'right' },
  ],
  project: [
    { id: 'task-list',   labelKey: 'pageGuide.projectTaskList',   top: '50%', left: '50%' },
    { id: 'shortcuts',   labelKey: 'pageGuide.projectShortcuts',  top: '20%', left: '55%', arrowDirection: 'up' },
    { id: 'more-menu',   labelKey: 'pageGuide.projectMoreMenu',   top: '20%',  left: '95%', arrowDirection: 'up' },
  ],
  task: [
    { id: 'view-switch', labelKey: 'pageGuide.taskViewSwitch',  top: '33%', left: '77%', arrowDirection: 'up' },
    { id: 'material',    labelKey: 'pageGuide.taskMaterial',     top: '60%', left: '50%' },
    { id: 'sidebar',     labelKey: 'pageGuide.taskSidebar',      top: '60%', left: '88%' },
    { id: 'workflow',    labelKey: 'pageGuide.taskWorkflow',     top: '20%', left: '70%', arrowDirection: 'up' },
  ],
  taskList: [
    { id: 'enable-tab',   labelKey: 'pageGuide.taskListEnable',   top: '27%', left: '50%', arrowDirection: 'left' },

  ],
  gameIntro: [
    { id: 'files',     labelKey: 'pageGuide.gameIntroFiles',     top: '50%', left: '50%' },
    { id: 'prototype', labelKey: 'pageGuide.gameIntroPrototype',  top: '20%', left: '80%', arrowDirection: 'up' },
  ],
  materials: [
    { id: 'groups',    labelKey: 'pageGuide.materialsGroups',    top: '50%', left: '50%' },
  ],
  scale: [
    { id: 'select',  labelKey: 'pageGuide.scaleSelect',  top: '50%', left: '40%' },
    { id: 'panel',   labelKey: 'pageGuide.scalePanel',   top: '50%', left: '70%', arrowDirection: 'right' },
  ],
  convert: [
    { id: 'images',    labelKey: 'pageGuide.convertImages',    top: '50%', left: '50%' },
  ],
  settings: [
    { id: 'tabs',     labelKey: 'pageGuide.settingsTabs',     top: '40%', left: '15%', arrowDirection: 'left' },
    { id: 'content',  labelKey: 'pageGuide.settingsContent',  top: '40%', left: '55%' },
  ],
  settingsAttendance: [
    { id: 'att-config', labelKey: 'pageGuide.attConfig', top: '40%', left: '53%' },
    { id: 'att-more',   labelKey: 'pageGuide.attMore',   top: '13.5%',  left: '77%', arrowDirection: 'right' },
  ],
}
