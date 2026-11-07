import { Link } from "@tanstack/react-router";

export function BackLink() {
  return (
    <div className="back">
      <Link to="/">Your resumes</Link>
    </div>
  );
}
