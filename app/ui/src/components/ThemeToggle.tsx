import React from 'react'

interface ThemeToggleProps {
  isDark: boolean
  onToggle: () => void
}

export const ThemeToggle: React.FC<ThemeToggleProps> = ({ isDark, onToggle }) => {
  return (
    <button className="m3-display-capsule" onClick={onToggle}>
      <span>{isDark ? '☀️' : '🌙'}</span>
      <span>{isDark ? '浅色模式' : '夜间深色'}</span>
    </button>
  )
}
