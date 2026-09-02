import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import ObsidianSync from './components/ObsidianSync'
import './styles.css'
import './m2.css'
import './m3.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <ObsidianSync />
  </StrictMode>,
)
