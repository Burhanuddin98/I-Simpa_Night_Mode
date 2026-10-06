import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
// Decision 60: the fonts ship with the app (OFL; their licences in public/licenses/).
import '@fontsource-variable/inter/index.css';
import '@fontsource-variable/jetbrains-mono/index.css';
import './theme.css';
import { App } from './App';

const root = document.getElementById('root');
if (!root) throw new Error('#root is missing from index.html');
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
