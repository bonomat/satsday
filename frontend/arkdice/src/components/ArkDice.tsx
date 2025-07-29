import { useEffect, useState } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Copy, Dice1 } from 'lucide-react'

interface GameAddress {
  address: string
  max_roll: number
  multiplier: string
  multiplier_value: number
  win_probability: number
}

interface GameData {
  game_addresses: GameAddress[]
  info: {
    roll_range: string
    win_condition: string
  }
}

export function ArkDice() {
  const [gameData, setGameData] = useState<GameData | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [copiedAddress, setCopiedAddress] = useState<string | null>(null)

  useEffect(() => {
    const fetchGameData = async () => {
      try {
        const response = await fetch('http://localhost:12345/game-addresses')
        if (!response.ok) {
          throw new Error('Failed to fetch game data')
        }
        const data = await response.json()
        setGameData(data)
      } catch (err) {
        setError(err instanceof Error ? err.message : 'An error occurred')
      } finally {
        setLoading(false)
      }
    }

    fetchGameData()
  }, [])

  const copyToClipboard = async (address: string) => {
    try {
      await navigator.clipboard.writeText(address)
      setCopiedAddress(address)
      setTimeout(() => setCopiedAddress(null), 2000)
    } catch (err) {
      console.error('Failed to copy:', err)
    }
  }

  const getMultiplierColor = (multiplier: number) => {
    if (multiplier >= 10000) return 'text-purple-600 dark:text-purple-400'
    if (multiplier >= 5000) return 'text-red-600 dark:text-red-400'
    if (multiplier >= 1000) return 'text-orange-600 dark:text-orange-400'
    if (multiplier >= 300) return 'text-yellow-600 dark:text-yellow-400'
    if (multiplier >= 200) return 'text-green-600 dark:text-green-400'
    return 'text-blue-600 dark:text-blue-400'
  }

  const getProbabilityColor = (probability: number) => {
    if (probability >= 80) return 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
    if (probability >= 50) return 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200'
    if (probability >= 20) return 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200'
    if (probability >= 5) return 'bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-200'
    return 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200'
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary"></div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <Card className="w-96">
          <CardHeader>
            <CardTitle className="text-red-600">Error</CardTitle>
            <CardDescription>{error}</CardDescription>
          </CardHeader>
        </Card>
      </div>
    )
  }

  if (!gameData) return null

  return (
    <div className="container mx-auto p-4 max-w-7xl">
      <Card className="mb-8">
        <CardHeader className="text-center">
          <div className="flex items-center justify-center gap-2 mb-2">
            <Dice1 className="h-8 w-8 text-primary" />
            <CardTitle className="text-4xl font-bold bg-gradient-to-r from-primary to-primary/60 bg-clip-text text-transparent">
              ArkDice
            </CardTitle>
          </div>
          <CardDescription className="text-lg">
            Send sats to any address below to play. Win if dice rolls under the target!
          </CardDescription>
          <div className="mt-4 flex flex-col gap-2 text-sm text-muted-foreground">
            <div>🎲 Roll Range: {gameData.info.roll_range}</div>
            <div>✅ Win Condition: {gameData.info.win_condition}</div>
          </div>
        </CardHeader>
      </Card>

      <Card>
        <CardContent className="p-0">
          <TooltipProvider>
            <Table>
              <TableHeader>
                <TableRow className="hover:bg-transparent">
                  <TableHead className="text-center">Multiplier</TableHead>
                  <TableHead className="text-center">Win Chance</TableHead>
                  <TableHead className="text-center">Max Roll</TableHead>
                  <TableHead>Send ARK To This Address</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {gameData.game_addresses.map((game) => (
                  <TableRow key={game.address} className="hover:bg-muted/50 transition-colors">
                    <TableCell className="text-center">
                      <span className={`text-2xl font-bold ${getMultiplierColor(game.multiplier_value)}`}>
                        {game.multiplier}
                      </span>
                    </TableCell>
                    <TableCell className="text-center">
                      <Badge className={getProbabilityColor(game.win_probability)}>
                        {game.win_probability.toFixed(2)}%
                      </Badge>
                    </TableCell>
                    <TableCell className="text-center font-mono">
                      {game.max_roll.toLocaleString()}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <code className="text-xs bg-muted p-2 rounded flex-1 truncate font-mono">
                          {game.address}
                        </code>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => copyToClipboard(game.address)}
                              className="shrink-0"
                            >
                              <Copy className="h-4 w-4" />
                              {copiedAddress === game.address && (
                                <span className="ml-2 text-xs">Copied!</span>
                              )}
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p>Copy address</p>
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TooltipProvider>
        </CardContent>
      </Card>

      <div className="mt-8 text-center text-sm text-muted-foreground">
        <p>🎰 Send any amount of ARK to play. Winnings are sent back instantly!</p>
        <p className="mt-2">⚡ Powered by Ark Protocol</p>
      </div>
    </div>
  )
}