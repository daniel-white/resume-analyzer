export function FileHeading({ filename }: { filename: string }) {
  return (
    <div className="filehead">
      <h1>{filename}</h1>
    </div>
  );
}
