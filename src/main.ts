import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import i18n from "./i18n";

/* 样式引入顺序: reset → 设计系统 → 工具类 → 组件公共 */
import "./styles/reset.css";
import "./styles/design-system.css";
import "./styles/glass.css";
import "./styles/dialog.css";

createApp(App).use(router).use(i18n).mount("#app");
