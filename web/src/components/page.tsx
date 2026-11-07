import type { ReactNode } from "react";

export function Page({
  children,
  wide = false,
}: {
  children: ReactNode;
  wide?: boolean;
}) {
  return <main className={wide ? "wrap wide" : "wrap"}>{children}</main>;
}
