export function FileHeading({ filename }: { filename: string }) {
  return (
    <div className="filehead">
      <h1 className="filename">{filename}</h1>
    </div>
  );
}
