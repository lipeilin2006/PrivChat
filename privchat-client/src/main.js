import { createApp } from "vue";
import { Quasar, Notify, Dialog, Dark, Screen } from "quasar";
import langZh from "quasar/lang/zh-CN";
import langEn from "quasar/lang/en-US";
import quasarIconSet from "quasar/icon-set/svg-material-icons";
import "@quasar/extras/material-icons/material-icons.css";
import "quasar/src/css/index.sass";
import App from "./App.vue";

const savedLocale = localStorage.getItem("privchat_locale");
const systemLocale = navigator.language || navigator.languages?.[0] || "en";
const quasarLocale = (savedLocale || (systemLocale.toLowerCase().startsWith("zh") ? "zh-CN" : "en")) === "en" ? langEn : langZh;

createApp(App)
  .use(Quasar, {
    plugins: { Notify, Dialog, Dark, Screen },
    iconSet: quasarIconSet,
    lang: quasarLocale,
    config: {
      brand: {
        primary: "#3390ec",
        secondary: "#2b5278",
        accent: "#6ab3f3",
        dark: "#17212b",
        "dark-page": "#0e1621",
        positive: "#4fae4e",
        negative: "#df3e3e",
        info: "#3390ec",
        warning: "#ef8f36",
      },
    },
  })
  .mount("#app");
