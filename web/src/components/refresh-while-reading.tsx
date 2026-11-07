import { useRouter } from "@tanstack/react-router";
import { useEffect } from "react";

export function RefreshWhileReading({ active }: { active: boolean }) {
  const router = useRouter();
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(() => {
      void router.invalidate();
    }, 2000);
    return () => clearInterval(timer);
  }, [active, router]);
  return null;
}
