# Pokemon TCG Scraper

## Deployment instructions
1. Log in to GitHub Container registry
`echo $PAT_TOKEN | docker login ghcr.io -u USERNAME --password-stdin`
Remember to remove the entry from zsh history so that you don't leak any credentials.

2. Build and push the project
```
docker compose build
docker compose push
```

3. Set the docker context to the remote server
`docker context use tardis`

4. Backup the database
`docker cp 7267d7516ccd:/usr/src/app/db/demo.db ./backup.db`

5. Deploy the application
`docker stack deploy pkmn -c docker-compose.yml`

6. Verify the deployment is running
`docker ps`


7. Change your docker context back
`docker context use default`
and verify that it has worked
`docker ps`
