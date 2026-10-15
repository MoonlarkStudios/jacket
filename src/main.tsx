import { MantineProvider } from '@mantine/core';
import React, { lazy, Suspense } from 'react';
import ReactDOM from 'react-dom/client';
import '@mantine/core/styles.css';
import './styles.css';
import { ApplicationBoundary } from './components/ApplicationBoundary';
import { jacketTheme } from './theme';

const Screen = lazy(() => import('./App'));

const root = document.getElementById('root');
if (!root) throw new Error('Jacket mount element is missing.');

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <MantineProvider theme={jacketTheme} forceColorScheme="light">
      <ApplicationBoundary>
        <Suspense
          fallback={
            <main className="startup-recovery" role="status">
              Opening Jacket…
            </main>
          }
        >
          <Screen />
        </Suspense>
      </ApplicationBoundary>
    </MantineProvider>
  </React.StrictMode>,
);
