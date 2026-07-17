import { useEffect, useState } from "react"
import { Toaster as Sonner } from "sonner"

type ToasterProps = React.ComponentProps<typeof Sonner>

/** Este proyecto no usa next-themes (no es una app Next.js) — el tema activo
 *  se lee directamente de la clase `.dark` en `<html>`, la misma fuente de
 *  verdad que usa `theme/index.ts` (`applyBaseTheme`). Un `MutationObserver`
 *  mantiene esto sincronizado si el usuario cambia de tema en Ajustes sin
 *  recargar la app. */
function useDarkMode(): boolean {
  const [isDark, setIsDark] = useState(() => document.documentElement.classList.contains("dark"))

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.classList.contains("dark"))
    })
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] })
    return () => observer.disconnect()
  }, [])

  return isDark
}

const Toaster = ({ ...props }: ToasterProps) => {
  const isDark = useDarkMode()

  return (
    <Sonner
      theme={isDark ? "dark" : "light"}
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg",
          description: "group-[.toast]:text-muted-foreground",
          actionButton:
            "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
          cancelButton:
            "group-[.toast]:bg-muted group-[.toast]:text-muted-foreground",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
