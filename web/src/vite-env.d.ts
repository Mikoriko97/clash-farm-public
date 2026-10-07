/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** "1" — збірка містить мок-режим (staging, `?mock=1`); у prod не задавати. */
  readonly VITE_MOCK_API?: string;
}
