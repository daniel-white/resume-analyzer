import { Link } from "@tanstack/react-router";
import { ArrowUpRight, BookOpen } from "lucide-react";

export function PublicHeader() {
  return (
    <header className="site-navigation public-navigation">
      <Link to="/" className="landing-brand"><span><BookOpen size={23} aria-hidden="true" /></span>The Other Read<span className="landing-beta">BETA</span></Link>
      <Link to="/" className="public-home">Back to your next move <ArrowUpRight size={16} aria-hidden="true" /></Link>
    </header>
  );
}
