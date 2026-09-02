import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import ObsidianSync from './components/ObsidianSync'
import EndOfDay from './components/EndOfDay'
import NotesWorkspace from './components/NotesWorkspace'
import './styles.css'
import './m2.css'
import './m3.css'
import './m4.css'
import './m5.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <ObsidianSync />
    <EndOfDay />
    <NotesWorkspace />
  </StrictMode>,
)
