import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import { Bubble } from './bubble/Bubble';

const container = document.getElementById('root');
if (!container) throw new Error('missing #root');

createRoot(container).render(
  <StrictMode>
    <Bubble />
  </StrictMode>,
);
