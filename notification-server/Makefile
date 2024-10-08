.DEFAULT_GOAL := all

all: docker-up wait-for-db create-tables

setup: setup-db setup-credentials

setup-db:
	@echo "Setting default password"
	@DB_PASSWORD=$$(openssl rand -base64 12) && \
	export DB_PASSWORD=$$DB_PASSWORD && \
	cat .env.template | envsubst > .env

setup-credentials:
	@echo "Please enter the path to your Google application credentials file:"
	read GOOGLE_APPLICATION_CREDENTIALS_PATH && \
	mkdir -p secrets && \
	cp $$GOOGLE_APPLICATION_CREDENTIALS_PATH secrets/google-credentials.json

docker-up:
	@echo "Starting Docker containers..."
	docker compose up -d db

# Target to wait for the PostgreSQL database to be ready.
wait-for-db:
	@echo "Waiting for the database to be ready..."
	@max_attempts=10; \
	current_attempt=1; \
	until docker compose exec -T db pg_isready -U postgres; do \
		sleep 5; \
		current_attempt=$$((current_attempt+1)); \
		if [ $$current_attempt -gt $$max_attempts ]; then \
			echo "Database did not become ready in time."; \
			exit 1; \
		fi; \
		echo "Retrying ($$current_attempt/$$max_attempts)..."; \
	done

create-tables:
	@echo "Creating tables from SQL files..."
	@for sql_file in resources/*.sql; do \
		echo "Processing $$sql_file..."; \
		docker compose exec -T db psql --username=postgres < $$sql_file; \
	done

.PHONY: setup setup-credential setup-db all docker-up wait-for-db create-tables
