export function Icon({ name }: { name: 'workspace' | 'connections' | 'new-connection' | 'settings' | 'sources' | 'trace' | 'topology' | 'search' | 'close' | 'journal' | 'docker' | 'kubernetes' | 'chevron-down' | 'chevron-right' }) {
  const paths: Record<typeof name, string> = {
    workspace: 'M3 4h18v16H3z M3 9h18 M8 9v11',
    connections: 'M4 4h16v6H4z M4 14h16v6H4z M7 7h.01 M7 17h.01',
    'new-connection': 'M4 4h16v6H4z M4 14h16v6H4z',
    settings: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M12 3v2 M12 19v2 M3 12h2 M19 12h2 M5.6 5.6 7 7 M17 17l1.4 1.4 M5.6 18.4 7 17 M17 7l1.4-1.4',
    sources: 'M4 3h10l6 6v12H4z M14 3v6h6 M8 13h8 M8 17h6',
    trace: 'M4 5h16 M4 12h11 M4 19h7 M8 3v4 M15 10v4 M10 17v4',
    topology: 'M9 3h6v6H9z M3 15h6v6H3z M15 15h6v6h-6z M12 9v3 M6 15v-3h12v3',
    search: 'M10.5 3a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15z M16 16l5 5',
    journal: 'M5 3h14v18H5z M8 7h8 M8 11h8 M8 15h5',
    docker: 'M4 8h16v12H4z M8 8V4h8v4 M8 12h2 M14 12h2 M8 16h8',
    kubernetes: 'M12 3 21 8v8l-9 5-9-5V8z M12 3v9 M3 8l9 4 9-4 M12 12v9',
    'chevron-down': 'm6 9 6 6 6-6',
    'chevron-right': 'm9 6 6 6-6 6',
    close: 'm6 6 12 12 M18 6 6 18',
  };
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={paths[name]}/></svg>;
}

