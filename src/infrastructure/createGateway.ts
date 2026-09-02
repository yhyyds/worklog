import type { WorklogGateway } from '../application/gateway'
import { BrowserGateway } from './browserGateway'
import { DesktopGateway } from './desktopGateway'

export function createGateway(): WorklogGateway {
  return '__TAURI_INTERNALS__' in window ? new DesktopGateway() : new BrowserGateway()
}
