FROM python:3.12-slim-bookworm
WORKDIR /srv
COPY __init__.py __main__.py stub.py /srv/providers/
ENV PYTHONUNBUFFERED=1 PYTHONDONTWRITEBYTECODE=1
EXPOSE 8080
ENTRYPOINT ["python", "-m", "providers"]
