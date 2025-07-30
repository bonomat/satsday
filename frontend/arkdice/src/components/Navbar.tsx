import { Button } from "@/components/ui/button";
import { Shield, HelpCircle, Home as HomeIcon } from "lucide-react";

const Navbar = () => {
  return (
    <nav className="bg-gray-800 border-b border-gray-700 px-6 py-4">
      <div className="max-w-7xl mx-auto flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <HomeIcon className="h-6 w-6 text-orange-500" />
          <span className="text-xl font-bold text-orange-500">
            Satoshi Dice
          </span>
        </div>
        <div className="flex items-center space-x-4">
          <Button variant="ghost" className="text-gray-300 hover:text-white">
            <HelpCircle className="h-4 w-4 mr-2" />
            FAQ
          </Button>
          <Button variant="ghost" className="text-gray-300 hover:text-white">
            <Shield className="h-4 w-4 mr-2" />
            Provably Fair
          </Button>
        </div>
      </div>
    </nav>
  );
};

export default Navbar;