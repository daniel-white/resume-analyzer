import { Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";

export function BackLink() {
  return (
    <div className="back">
      <Link to="/"><ArrowLeft size={15} aria-hidden="true" /> Your résumés</Link>
    </div>
  );
}
