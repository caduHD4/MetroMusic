# Changelog (Português)

[English](CHANGELOG.md) · **Português (Brasil)**

Cada versão publicada no GitHub usa o texto da sua seção como "o que mudou" (é o que aparece no aviso de atualização dentro do app).

## 0.1.10

Correção do Discord no Windows.

### Discord
- **A presença no Discord agora encontra o Discord no Windows.** O app procurava o arquivo de socket do Linux, mas no Windows o Discord usa um named pipe (`\.\pipe\discord-ipc-N`), então sempre dizia "não achei o Discord aberto". Agora ele conecta ao pipe.

## 0.1.9

Agora você escolhe o idioma das legendas do YouTube.

### Letras
- **Novo "Idioma da legenda…" no menu da letra.** Ele lista as faixas de legenda que o vídeo tem (por exemplo inglês, japonês, português, espanhol), marca a recomendada, separa as escritas por uma pessoa das automáticas e mostra qual está em uso. A sua escolha fica salva para aquela música, e o crédito da letra agora diz o idioma ("YouTube Legenda · Portuguese").
- **Nova opção: Configurações › Provedores de letra › "Idioma da legenda do YouTube".** Define o idioma que a escolha automática usa quando o vídeo tem uma faixa escrita por uma pessoa nele; senão vale o idioma original da música.
- "Buscar de novo" apaga a sua escolha e volta à automática.

## 0.1.8

Mais correções nas letras vindas das legendas do YouTube.

### Letras
- **As legendas agora usam o idioma original da música.** Vídeos com várias faixas de legenda (inglês, espanhol, português…) davam a primeira que viesse, que podia ser uma tradução. O app agora escolhe o idioma pela escrita do título (kana → japonês, hangul → coreano…), ou pelo idioma da legenda automática, e depois o inglês. Para ler no seu idioma, use "Traduzir com IA" no menu da letra.
- **Chega de linhas repetidas.** Legendas animadas repetem o mesmo texto a cada quadro (um vídeo tinha 198 trechos para cerca de 50 versos). Uma repetição que começa até 1,5 s depois da anterior agora conta como a mesma linha. Versos que se repetem de verdade na música continuam.
- **Caracteres invisíveis** (espaços de largura zero) em volta do texto são removidos de todos os provedores de letra, assim como o aviso de abertura que alguns vídeos põem em 0 ms (como o de luzes piscantes).
- A letra do Spotify continua sem funcionar: o player web do Spotify mudou e a consulta falha. É um problema separado.

## 0.1.7

Correção nas letras.

### Letras
- **As legendas do YouTube não mostram mais tags HTML.** Algumas legendas chegavam como `<font color="…"><b>palavra</b></font>` no meio da letra. As tags agora são removidas de todos os provedores de letra, não só das legendas.

## 0.1.6

Letras que escolhem a melhor fonte sozinhas, e as ferramentas de letra agora funcionam de verdade.

### Letras
- **O app agora escolhe a melhor letra sozinho.** Ele pergunta a todos os provedores ligados ao mesmo tempo e fica com o melhor resultado: palavra a palavra vence linha a linha, que vence sem sincronia. A ordem das configurações só desempata. Letra cujo tempo passa do fim da faixa (outra versão da música) perde pontos. Cada provedor tem 8 segundos.
- **Provedores novos**, portados do MetroFuse: Apple Music (Paxsenix), Musixmatch, LyricsPlus, Binimum, QQ Music, YouTube Music e Deezer (precisa da conta). Eles se somam a Better Lyrics, LrcLib, KuGou e Spotify.
- **Legendas do YouTube** agora funcionam como fonte de letra, linha a linha, usando as legendas do próprio vídeo.
- O Musixmatch só é aceito quando o título e o artista que ele casou são os que você pediu (antes podia devolver a letra de outra música).
- **"Buscar de novo"** pergunta de verdade a todos os provedores outra vez. **"Buscar outra letra…"** lista o que cada provedor achou, com o tipo de sincronia, para você escolher. **"Editar letra"** salva (LRC com tempos, ou texto simples). **"Compartilhar como imagem"** cria um PNG de verdade, para copiar ou salvar. O ajuste de sincronia e as letras que você escolhe ou edita ficam guardados por faixa.
- Os downloads com a letra embutida agora seguem a sua ordem de provedores (era fixa em Better Lyrics + LrcLib).
- Nova opção: *Musixmatch: só linha a linha*.
- Sem resultado em algumas fontes no momento: o Apple Music depende de um serviço de token de terceiros que estava fora do ar nos testes, e as letras do Spotify e do Deezer pedem a conta conectada.

### Fontes de áudio
- **O Deezer agora toca pela sua conta** (cookie `arl`): faixas inteiras, em FLAC com conta premium.
- TIDAL e JioSaavn foram removidos.

## 0.1.5

O áudio agora acompanha o dispositivo de saída do sistema.

### Áudio
- **Trocar o dispositivo de saída no Windows agora funciona com o app aberto.** O MetroMusic segue o padrão do sistema (fones, caixas, HDMI, Bluetooth…) e o som vai junto. Antes, ele ficava no dispositivo que estava selecionado quando o app abriu. Se o novo dispositivo usa outra taxa de amostragem, a faixa atual recarrega do mesmo ponto e continua pausada se estava pausada.
- No Linux nada muda: o PipeWire/PulseAudio já movem o som (conferido trocando o dispositivo padrão com o motor de áudio tocando).

## 0.1.4

Correções na reprodução ao abrir o app e no Ouvir junto.

### Reprodução
- **A música não começa mais "pausada e sem som" ao abrir o app.** A fila restaurada agora é carregada de verdade ao apertar play, e retoma de onde você parou. O mesmo vale para uma faixa que acabou com o autoplay desligado.
- Pausar, retomar ou buscar enquanto a faixa ainda está carregando agora vale: antes o pedido se perdia e a música começava sozinha, do zero.

### Ouvir junto
- Quem entra no meio da música agora começa **na posição do anfitrião** (antes tocava do 0).
- Quando alguém entra na sala, a música **não reinicia mais** para quem já estava nela.
- O anfitrião avisa quando carregou a faixa, então a sala começa a tocar logo, sem esperar 8 segundos a cada troca.
- A posição segue o relógio do anfitrião: se a faixa demora a carregar, ela pula para onde ele está, e uma checagem periódica corrige a diferença com o tempo.
- Testado de ponta a ponta contra o servidor real (`pnpm test:net`), com o app como anfitrião e como convidado.

### Documentação
- README e changelog agora são em inglês por padrão, com a versão em português ao lado.

## 0.1.3

Nova opção de fundo.

### Aparência
- Nova opção de fundo **"Ada"** no app e no Tocando agora (contribuição de Helon21).

## 0.1.2

O AppImage do Linux agora usa a placa de vídeo.

### Linux
- **O AppImage é compilado no Ubuntu 24.04**, com o WebKit 2.52 embutido (antes vinha um WebKit mais antigo que, em alguns sistemas como CachyOS com placa NVIDIA, desenhava tudo pela CPU e deixava a interface lenta). Com o novo, o WebKit inicializa a GPU.
- Em *Configurações › Aparência › Desempenho visual* agora aparece **qual placa de vídeo o app está usando** (ou "desenhando por software") e se o modo leve está ligado, para conferir na hora.
- Requisito do pacote: sistemas com glibc 2.39 ou mais novo (Ubuntu 24.04+, Debian 13+, Fedora 40+, Arch e derivados). Em distribuições mais antigas, compile a partir do código.

## 0.1.1

Correção de desempenho: o app agora roda fluido em qualquer PC, inclusive sem placa de vídeo.

### Desempenho
- **Modo leve automático:** se o PC não tem GPU (desenho por software) ou o app percebe lentidão, ele desliga sozinho os desfoques gigantes, as sombras grandes, as animações decorativas e os vídeos de fundo. Quem tem GPU continua com todos os efeitos, sem mudança.
- Nova opção em *Configurações › Aparência › Desempenho visual*: Automático (padrão), Completo ou Leve.
- No modo leve, o fundo desfocado do Tocando agora vira um gradiente com a cor da capa, e a letra anima a 30 quadros por segundo.
- Em qualquer PC: as animações pausam quando a janela está escondida ou sem foco, e o desfoque da letra só vale nas linhas próximas à atual.
- Testado desenhando por software: o custo de CPU do Início caiu de 15% para 3% de um núcleo, e o do Tocando agora com desfoque em movimento, de 117% para 16%.

### Linux
- O AppImage traz o seu próprio WebKit e, em alguns sistemas (por exemplo com placa NVIDIA), ele não usa a GPU. O modo leve cobre esse caso. Quem prefere o WebKit do sistema pode usar o pacote `.deb` ou compilar a partir do código.

## 0.1.0

Primeira versão pública do MetroMusic.

### Ouvir música
- YouTube Music e SoundCloud tocam direto no app; escolha de fonte por faixa mostrando só resultados válidos.
- Qualidade do áudio configurável, equalizador de 10 bandas, normalização, crossfade e reprodução sem pausa entre faixas.
- Timer de sono, alarmes e sono automático.

### Letras
- Letras sincronizadas palavra a palavra (BetterLyrics, LrcLib, KuGou, Spotify), com romanização (japonês, coreano, chinês, cirílico, hindi e punjabi) e tradução por IA.

### Contas e serviços
- Entrar no YouTube Music, Spotify, SoundCloud e Last.fm; curtidas e playlists sincronizadas.
- **Discord Rich Presence** sem configurar nada: mostra a música, o tempo e a pausa no seu perfil.
- **Ouvir junto** com salas compatíveis com o MetroFuse e o Metrolist.
- Spotify Canvas sobre a capa (com a conta do Spotify conectada).

### Biblioteca
- Downloads com título, artista, álbum, capa e letra gravados no arquivo.
- Biblioteca local: varre suas pastas, lê as tags e acompanha mudanças.
- Importar playlists de M3U, CSV e links do YouTube Music e do Spotify.
- Backup e restauração.

### Aparência
- Temas, paletas, fundos "Galáxia" e desfocados, idiomas português e inglês.

### Atualizações
- O app avisa quando há versão nova, mostra o que mudou e atualiza com um clique, sem instalador.
