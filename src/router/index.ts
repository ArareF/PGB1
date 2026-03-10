import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('../views/HomePage.vue'),
    },
    {
      path: '/project/:projectId',
      name: 'project',
      component: () => import('../views/ProjectPage.vue'),
    },
    {
      path: '/project/:projectId/task/:taskId',
      name: 'task',
      component: () => import('../views/TaskPage.vue'),
    },
    {
      path: '/project/:projectId/task/:taskId/scale',
      name: 'scale',
      component: () => import('../views/ScalePage.vue'),
    },
    {
      path: '/project/:projectId/task/:taskId/convert',
      name: 'convert',
      component: () => import('../views/ConvertPage.vue'),
    },
    {
      path: '/project/:projectId/task-list',
      name: 'taskList',
      component: () => import('../views/TaskListPage.vue'),
    },
    {
      path: '/project/:projectId/game-intro',
      name: 'gameIntro',
      component: () => import('../views/GameIntroPage.vue'),
    },
    {
      path: '/project/:projectId/materials',
      name: 'materials',
      component: () => import('../views/MaterialsPage.vue'),
    },
    {
      path: '/reminder/:type',
      name: 'reminder',
      component: () => import('../views/ReminderPage.vue'),
    },
    {
      path: '/overtime',
      name: 'overtime',
      component: () => import('../views/OvertimePage.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: () => import('../views/SettingsPage.vue'),
    },
    {
      path: '/translator',
      name: 'translator',
      component: () => import('../views/TranslatorPage.vue'),
    },
    {
      path: '/pinboard',
      name: 'pinboard',
      component: () => import('../views/PinboardPage.vue'),
    },
  ],
})

export default router
