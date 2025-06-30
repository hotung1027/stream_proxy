module github.com/your-org/camera-stream-proxy

go 1.21

require (
	// Web framework and middleware
	github.com/gin-gonic/gin v1.9.1
	github.com/gin-contrib/cors v1.5.0
	github.com/gin-contrib/static v0.0.1
	
	// Configuration management
	gopkg.in/yaml.v3 v3.0.1
	github.com/spf13/viper v1.17.0
	github.com/spf13/cobra v1.8.0
	
	// Authentication and security
	github.com/golang-jwt/jwt/v5 v5.2.0
	golang.org/x/crypto v0.17.0
	
	// Database and storage
	github.com/go-redis/redis/v8 v8.11.5
	github.com/google/uuid v1.5.0
	
	// Networking and protocols
	github.com/gorilla/websocket v1.5.1
	github.com/pion/webrtc/v3 v3.2.26
	
	// Logging and monitoring
	github.com/sirupsen/logrus v1.9.3
	github.com/prometheus/client_golang v1.17.0
	
	// Utilities
	github.com/go-playground/validator/v10 v10.16.0
	github.com/stretchr/testify v1.8.4
	golang.org/x/sync v0.5.0
	golang.org/x/time v0.5.0
)

require (
	github.com/bytedance/sonic v1.9.1 // indirect
	github.com/chenzhuoyu/base64x v0.0.0-20221115062448-fe3a3abad311 // indirect
	github.com/gabriel-vasile/mimetype v1.4.2 // indirect
	github.com/gin-contrib/sse v0.1.0 // indirect
	github.com/go-playground/locales v0.14.1 // indirect
	github.com/go-playground/universal-translator v0.18.1 // indirect
	github.com/goccy/go-json v0.10.2 // indirect
	github.com/json-iterator/go v1.1.12 // indirect
	github.com/klauspost/cpuid/v2 v2.2.4 // indirect
	github.com/leodido/go-urn v1.2.4 // indirect
	github.com/mattn/go-isatty v0.0.19 // indirect
	github.com/modern-go/concurrent v0.0.0-20180306012644-bacd9c7ef1dd // indirect
	github.com/modern-go/reflect2 v1.0.2 // indirect
	github.com/pelletier/go-toml/v2 v2.0.8 // indirect
	github.com/twitchyliquid64/golang-asm v0.15.1 // indirect
	github.com/ugorji/go/codec v1.2.11 // indirect
	golang.org/x/arch v0.3.0 // indirect
	golang.org/x/net v0.17.0 // indirect
	golang.org/x/sys v0.15.0 // indirect
	golang.org/x/text v0.14.0 // indirect
		google.golang.org/protobuf v1.31.0 // indirect
) 