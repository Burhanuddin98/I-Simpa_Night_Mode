import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
// Decision 60: the fonts ship with the app (OFL; their licences in public/licenses/).
import '@fontsource-variable/inter/index.css';
import '@fontsource-variable/jetbrains-mono/index.css';
import './theme.css';
import { App } from './App';
// Last, so its transitions sit over the packages' own rules.
import './motion.css';
import './identity.css';
import './hardware.css';
import './depth.css';
import { installTilt } from './chrome/tilt';

// Under WebDriver nothing in the chrome moves (motion.css): the gates measure panels and compare frames.
if (navigator.webdriver) document.documentElement.setAttribute('data-no-motion', '');
installTilt();

const root = document.getElementById('root');
if (!root) throw new Error('#root is missing from index.html');
createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
