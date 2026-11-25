import { Button } from '../../shared/ui/Controls';
import { useState } from 'react';
import type { AppStatus } from '../../shared/contracts/generated';
import { AiSettings } from './AiSettings';
import { ServiceMappings } from './ServiceMappings';
import './tools.css';

type SettingsTab = 'provider' | 'access' | 'mappings';
const tabs: { id: SettingsTab; title: string; description: string }[] = [
  { id: 'provider', title: 'AI provider', description: 'Explain Error settings' },
  { id: 'access', title: 'MCP access', description: 'Coding clients and grants' },
  { id: 'mappings', title: 'Service mappings', description: 'Services and local code' },
];

export function Settings({ status }: { status?: AppStatus }) {
  const [tab, setTab] = useState<SettingsTab>('provider');
  return <div className="settings-layout">
    <nav className="settings-navigation" aria-label="Settings sections">{tabs.map(item => <Button key={item.id} className={tab === item.id ? 'active' : ''} aria-current={tab === item.id ? 'page' : undefined} onClick={() => setTab(item.id)}><strong>{item.title}</strong><small>{item.description}</small></Button>)}</nav>
    <div className="settings-content" id="settings-content">{tab === 'mappings' ? <ServiceMappings status={status}/> : <AiSettings key={tab} status={status} section={tab}/>}</div>
  </div>;
}
