# Page de maintenance — exemple WebCore

Une page « site en maintenance » **statique** (aucun JavaScript), compilée par
WebCore en HTML/CSS. Idéale à servir pendant une mise à jour ou une coupure
planifiée.

## Build

```bash
webc build examples/maintenance          # dev
webc build examples/maintenance --prod   # minifié, prêt pour la prod
```

Le résultat est dans `dist/` (`index.html` + `assets/`).

## Personnalisation

- **Texte** : `src/pages/home.webc` (titre, message, adresse de contact).
- **Couleurs / police** : `theme.toml`.
- **Style & animations** : `public/styles.css` (respecte
  `prefers-reduced-motion`).

## Déploiement : renvoyer un code HTTP 503

Une vraie page de maintenance doit répondre avec le statut **503 Service
Unavailable** (et non 200), pour que les moteurs de recherche ne
désindexent pas le site et reviennent plus tard. Ajoutez si possible un
en-tête `Retry-After`.

**nginx** — rediriger tout le trafic vers la page tant qu'elle existe :

```nginx
location / {
    if (-f $document_root/maintenance-on) {
        return 503;
    }
    try_files $uri $uri/ /index.html;
}

error_page 503 @maintenance;
location @maintenance {
    root /var/www/maintenance/dist;
    rewrite ^ /index.html break;
    add_header Retry-After 3600;
}
```

Activez/désactivez la maintenance avec un simple fichier témoin :
`touch maintenance-on` / `rm maintenance-on`.

**Apache** (`.htaccess`) :

```apache
RewriteEngine On
RewriteCond %{DOCUMENT_ROOT}/maintenance-on -f
RewriteCond %{REQUEST_URI} !=/index.html
RewriteRule ^ /index.html [R=503,L]
ErrorDocument 503 /index.html
Header always set Retry-After "3600"
```
