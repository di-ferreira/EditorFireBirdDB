import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";

// Ponto de entrada do frontend. O React é carregado como dependência explícita
// (não via global) para manter o JSX tipado pelo TypeScript.
const raiz = document.getElementById("root");
if (!raiz) {
  throw new Error("Elemento #root não encontrado no index.html");
}

createRoot(raiz).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
