import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import ObsidianSync from './components/ObsidianSync'
import EndOfDay from './components/EndOfDay'
import NotesWorkspace from './components/NotesWorkspace'
import DesktopIntegration from './components/DesktopIntegration'
import SettingsWorkspace from './components/SettingsWorkspace'
import GrowthWorkspace from './components/GrowthWorkspace'
import WeeklyReport from './components/WeeklyReport'
import InboxWorkspace from './components/InboxWorkspace'
import { applyFontScale, loadFontScale } from './application/appearance'
import './styles.css'
import './m2.css'
import './m3.css'
import './m4.css'
import './m5.css'
import './m6.css'
import './m7.css'
import './m8.css'
import './m10.css'
import './m11.css'
import './m12.css'
import './m13.css'
import './m14.css'

applyFontScale(loadFontScale())

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
    <ObsidianSync />
    <EndOfDay />
    <NotesWorkspace />
    <DesktopIntegration />
    <SettingsWorkspace />
    <GrowthWorkspace />
    <WeeklyReport />
    <InboxWorkspace />
  </StrictMode>,
)
