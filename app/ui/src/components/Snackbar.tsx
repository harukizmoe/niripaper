import React from 'react'

interface SnackbarProps {
  message: string
  visible: boolean
}

export const Snackbar: React.FC<SnackbarProps> = ({ message, visible }) => {
  return (
    <div className={`m3-snackbar ${visible ? 'show' : ''}`} id="snackbar">
      {message}
    </div>
  )
}
